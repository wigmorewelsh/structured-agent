import asyncio
import os
import subprocess
import tempfile
from pathlib import Path

import pytest
from dotenv import load_dotenv

import acp

load_dotenv(Path(__file__).parent.parent.parent / ".env")

PROJECT_ROOT = Path(__file__).parent.parent.parent


def build_binary():
    import subprocess
    result = subprocess.run(
        ["cargo", "build", "--bin", "structured-agent"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Failed to build binary: {result.stderr}")
    return PROJECT_ROOT / "target" / "debug" / "structured-agent"


@pytest.fixture(scope="session")
def binary_path():
    return build_binary()


@pytest.fixture(scope="session")
def gemini_api_key():
    key = os.environ.get("GEMINI_API_KEY")
    if not key:
        pytest.skip("GEMINI_API_KEY not set")
    return key


class ResponseCollector(acp.Client):
    def __init__(self):
        self.events = []
        self.got_event = asyncio.Event()

    async def session_update(self, session_id: str, update, **kwargs):
        if isinstance(update, acp.schema.AgentMessageChunk):
            content = update.content
            if hasattr(content, "text") and content.text.strip():
                self.events.append(content.text.strip())
                self.got_event.set()

    async def request_permission(self, options, session_id: str, tool_call, **kwargs):
        pass


SAMPLES_DIR = PROJECT_ROOT / "src" / "structured-agent" / "samples"


class AgentEventCollector(acp.Client):
    def __init__(self, expected_text: str | None = None):
        self.all_events = []
        self.tool_calls = []
        self.started = asyncio.Event()
        self.agent_ready = asyncio.Event()
        self.task_sent = False
        self.events_after_task = []
        self.got_response_after_task = asyncio.Event()
        self.expected_text = expected_text
        self.task_complete = asyncio.Event()

    async def session_update(self, session_id: str, update, **kwargs):
        if isinstance(update, acp.schema.AgentMessageChunk):
            content = update.content
            if hasattr(content, "text"):
                text = content.text.strip()
                if text:
                    self.all_events.append(("text", text))
                    if "Starting agent loop" in text:
                        self.started.set()
                    if self.task_sent:
                        self.events_after_task.append(text)
                        self.got_response_after_task.set()
                        if self.expected_text and self.expected_text in text:
                            self.task_complete.set()
        elif isinstance(update, acp.schema.ToolCall):
            self.tool_calls.append(update)
            self.all_events.append(("tool_call", update))
            if self.task_sent:
                self.events_after_task.append(("tool_call", update))
                self.got_response_after_task.set()
        elif isinstance(update, acp.schema.ToolCallUpdate):
            self.all_events.append(("tool_update", update))
            self.agent_ready.set()
            if self.task_sent:
                self.events_after_task.append(("tool_update", update))
                self.got_response_after_task.set()

    async def request_permission(self, options, session_id: str, tool_call, **kwargs):
        pass


PROGRAM = """
extern fn missing_tool(input: String): String

fn main(): () {
    missing_tool("hello")
}
"""


@pytest.mark.asyncio
@pytest.mark.timeout(180)
async def test_agent_sa_with_config_toml(binary_path, gemini_api_key):
    collector = AgentEventCollector(expected_text="BANANA")
    env = {**os.environ, "GEMINI_API_KEY": gemini_api_key}
    config_path = SAMPLES_DIR / "config.toml"
    failure_reason = None
    fake_file_content = "The secret word is: BANANA"

    with tempfile.TemporaryDirectory() as work_dir:
        Path(work_dir, "history.md").write_text("")
        Path(work_dir, "secret.txt").write_text(fake_file_content)

        async with acp.spawn_agent_process(
            lambda agent: collector,
            str(binary_path),
            "acp",
            "--config", str(config_path),
            cwd=PROJECT_ROOT,
            transport_kwargs={"stderr": subprocess.PIPE},
            env=env,
        ) as (conn, process):
            await conn.initialize(
                protocol_version=acp.PROTOCOL_VERSION,
                client_info=acp.schema.Implementation(name="test", version="1.0"),
            )

            session = await conn.new_session(cwd=work_dir, mcp_servers=[])

            try:
                await asyncio.wait_for(collector.agent_ready.wait(), timeout=60)
            except asyncio.TimeoutError:
                failure_reason = (
                    f"Agent never became ready (read_file did not complete) within 60s.\n"
                    f"Events so far: {collector.all_events}"
                )

            if failure_reason is None:
                collector.task_sent = True
                await conn.prompt(
                    prompt=[acp.text_block("Read the file secret.txt and tell me what the secret word is.")],
                    session_id=session.session_id,
                )

                try:
                    await asyncio.wait_for(collector.task_complete.wait(), timeout=120)
                except asyncio.TimeoutError:
                    failure_reason = (
                        f"Agent never reported the secret word.\n"
                        f"All events: {collector.all_events}"
                    )

    stderr = (await process.stderr.read()).decode()

    print("\n--- All events ---")
    for event in collector.all_events:
        print(event)
    print("--- End events ---")

    if failure_reason:
        pytest.fail(f"{failure_reason}\nStderr:\n{stderr}")

    assert "panicked" not in stderr, f"Process panicked:\n{stderr}"
    response_text = " ".join(e for e in collector.events_after_task if isinstance(e, str))
    assert "BANANA" in response_text, (
        f"Expected agent to report the secret word. Got: {collector.events_after_task}"
    )


@pytest.mark.asyncio
@pytest.mark.timeout(120)
async def test_runtime_error_reported_when_extern_fn_has_no_provider(binary_path, gemini_api_key):
    collector = ResponseCollector()

    async with acp.spawn_agent_process(
        lambda agent: collector,
        str(binary_path),
        "acp",
        "--engine", "gemini",
        "--inline", PROGRAM,
        "--with-default-functions",
        "--with-acp-functions",
        cwd=PROJECT_ROOT,
        transport_kwargs={"stderr": None},
        env={"GEMINI_API_KEY": gemini_api_key},
    ) as (conn, process):
        await conn.initialize(
            protocol_version=acp.PROTOCOL_VERSION,
            client_info=acp.schema.Implementation(name="test", version="1.0"),
        )

        session = await conn.new_session(cwd=str(PROJECT_ROOT), mcp_servers=[])

        await asyncio.wait_for(collector.got_event.wait(), timeout=30)

        assert any("missing_tool" in e for e in collector.events), (
            f"Expected runtime error mentioning missing_tool. Got: {collector.events}"
        )
