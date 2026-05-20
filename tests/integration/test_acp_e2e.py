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
                if self.expected_text and update.content:
                    for c in update.content:
                        if hasattr(c, "content") and hasattr(c.content, "text"):
                            if self.expected_text in c.content.text:
                                self.task_complete.set()

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

    def extract_text(events):
        parts = []
        for e in events:
            if isinstance(e, str):
                parts.append(e)
            elif isinstance(e, tuple) and e[0] in ("tool_update", "tool_call"):
                update = e[1]
                if hasattr(update, "content") and update.content:
                    for c in update.content:
                        if hasattr(c, "content") and hasattr(c.content, "text"):
                            parts.append(c.content.text)
        return " ".join(parts)

    response_text = extract_text(collector.events_after_task)
    assert "BANANA" in response_text, (
        f"Expected agent to report the secret word. Got: {collector.events_after_task}"
    )


RESOURCE_LINK_PROGRAM = """
extern fn read_file(path: String): String
use io::print
use messaging::receive

fn extract_path(): String {
    "Extract the full absolute file path from the attached resource link and return only the path string, nothing else"!
}

fn main(): () {
    print("Starting agent loop...")
    let task = receive()
    task!
    let path = extract_path()
    let content = read_file(path)
    print(content)
}
"""


@pytest.mark.asyncio
@pytest.mark.timeout(60)
async def test_resource_link_path_resolved_to_full_path(binary_path, gemini_api_key):
    env = {**os.environ, "GEMINI_API_KEY": gemini_api_key}

    with tempfile.TemporaryDirectory() as work_dir:
        subdir = Path(work_dir) / "subdir" / "nested"
        subdir.mkdir(parents=True)
        secret_file = subdir / "secret.txt"
        secret_file.write_text("RASPBERRY")

        collector = AgentEventCollector(expected_text="RASPBERRY")

        async with acp.spawn_agent_process(
            lambda agent: collector,
            str(binary_path),
            "acp",
            "--engine", "gemini",
            "--inline", RESOURCE_LINK_PROGRAM,
            "--with-default-functions",
            "--with-acp-functions",
            "--mcp-server", "common-tools",
            cwd=PROJECT_ROOT,
            transport_kwargs={"stderr": subprocess.PIPE},
            env=env,
        ) as (conn, process):
            await conn.initialize(
                protocol_version=acp.PROTOCOL_VERSION,
                client_info=acp.schema.Implementation(name="test", version="1.0"),
            )

            session = await conn.new_session(cwd=work_dir, mcp_servers=[])

            await asyncio.wait_for(collector.started.wait(), timeout=30)

            collector.task_sent = True
            await conn.prompt(
                prompt=[
                    acp.resource_link_block(name="secret.txt", uri=secret_file.as_uri()),
                    acp.text_block("Read the attached file."),
                ],
                session_id=session.session_id,
            )

            try:
                await asyncio.wait_for(collector.task_complete.wait(), timeout=45)
            except asyncio.TimeoutError:
                pass

    stderr = ""
    if process.stderr is not None:
        stderr = (await process.stderr.read()).decode()

    print("\n--- All events ---")
    for event in collector.all_events:
        print(event)
    print("--- End events ---")

    assert "panicked" not in stderr, f"Process panicked:\n{stderr}"
    response_text = " ".join(e for e in collector.events_after_task if isinstance(e, str))
    assert "RASPBERRY" in response_text, (
        f"Expected agent to print file contents. Got: {collector.events_after_task}\nStderr: {stderr}"
    )


class CommandsCollector(acp.Client):
    def __init__(self):
        self.available_commands = []
        self.got_commands = asyncio.Event()
        self.session_ids_seen = []

    async def session_update(self, session_id: str, update, **kwargs):
        if isinstance(update, acp.schema.AvailableCommandsUpdate):
            self.session_ids_seen.append(session_id)
            self.available_commands.extend(update.available_commands)
            self.got_commands.set()

    async def request_permission(self, options, session_id: str, tool_call, **kwargs):
        pass


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_available_commands_received_after_new_session(binary_path):
    collector = CommandsCollector()

    async with acp.spawn_agent_process(
        lambda agent: collector,
        str(binary_path),
        "acp",
        "--engine", "print",
        "--inline", 'extern fn print(v: String): () fn main(): () { print("hi") }',
        cwd=PROJECT_ROOT,
        transport_kwargs={"stderr": subprocess.PIPE},
    ) as (conn, process):
        await conn.initialize(
            protocol_version=acp.PROTOCOL_VERSION,
            client_info=acp.schema.Implementation(name="test", version="1.0"),
        )

        session = await conn.new_session(cwd=str(PROJECT_ROOT), mcp_servers=[])

        try:
            await asyncio.wait_for(collector.got_commands.wait(), timeout=5)
        except asyncio.TimeoutError:
            pass

    assert collector.got_commands.is_set(), "AvailableCommandsUpdate was never received"
    assert session.session_id in collector.session_ids_seen, (
        f"Notification arrived for unknown session before new_session response. "
        f"Got session_ids: {collector.session_ids_seen}, expected: {session.session_id}"
    )
    command_names = [c.name for c in collector.available_commands]
    assert "reload" in command_names, f"Expected 'reload' command, got: {command_names}"


SELECT_STRUCT_PROGRAM = """
## A create task action
struct CreateTask { title: String }

## A complete task action
struct CompleteTask { id: String }

use io::print

fn main(): () {
    let result = select { CreateTask { title: "buy milk" }, CompleteTask { id: "42" } }
    print("select_done")
}
"""


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_select_with_struct_literal_clause(binary_path):
    collector = ResponseCollector()

    async with acp.spawn_agent_process(
        lambda agent: collector,
        str(binary_path),
        "acp",
        "--engine", "print",
        "--inline", SELECT_STRUCT_PROGRAM,
        cwd=PROJECT_ROOT,
        transport_kwargs={"stderr": subprocess.PIPE},
    ) as (conn, process):
        await conn.initialize(
            protocol_version=acp.PROTOCOL_VERSION,
            client_info=acp.schema.Implementation(name="test", version="1.0"),
        )
        await conn.new_session(cwd=str(PROJECT_ROOT), mcp_servers=[])
        try:
            await asyncio.wait_for(collector.got_event.wait(), timeout=15)
        except asyncio.TimeoutError:
            pass

    stderr = (await process.stderr.read()).decode()

    assert "panicked" not in stderr, f"Process panicked:\n{stderr}"
    assert any("select_done" in e for e in collector.events), (
        f"Expected 'select_done' output after select expression. "
        f"Got: {collector.events}\nStderr:\n{stderr}"
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
