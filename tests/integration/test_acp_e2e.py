import asyncio
import os
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


PROGRAM = """
extern fn missing_tool(input: String): String

fn main(): () {
    missing_tool("hello")
}
"""


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
