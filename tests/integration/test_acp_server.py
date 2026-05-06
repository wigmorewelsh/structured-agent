#!/usr/bin/env python3
"""
End-to-end integration tests for ACP server event streaming.

These tests verify the critical fix that events are sent to the ACP client
in real-time, not buffered until the agent completes.
"""

import asyncio
import subprocess
import tempfile
from pathlib import Path

import pytest
import acp


def build_binary():
    """Build the structured-agent binary before running tests."""
    project_root = Path(__file__).parent.parent.parent
    result = subprocess.run(
        ["cargo", "build", "--bin", "structured-agent"],
        cwd=project_root,
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        raise RuntimeError(f"Failed to build binary: {result.stderr}")
    return project_root / "target" / "debug" / "structured-agent"


@pytest.fixture(scope="session")
def binary_path():
    """Build and return path to the binary."""
    return build_binary()


class EventCollector(acp.Client):
    """Collects events and tracks whether they arrived before or after prompt was sent."""

    def __init__(self):
        self.events_before_prompt = []
        self.events_after_prompt = []
        self.prompt_sent = False
        self.event1_received = asyncio.Event()

    async def session_update(self, session_id: str, update, **kwargs):
        """Handle session updates from agent."""
        if isinstance(update, acp.schema.AgentMessageChunk):
            content = update.content
            if hasattr(content, 'text'):
                text = content.text.strip()
                if self.prompt_sent:
                    self.events_after_prompt.append(text)
                else:
                    self.events_before_prompt.append(text)
                if "Event 1" in text:
                    self.event1_received.set()

    async def request_permission(self, options, session_id: str, tool_call, **kwargs):
        """Required by Client protocol."""
        pass


@pytest.mark.asyncio
async def test_events_sent_before_receive_blocks(binary_path):
    """
    Critical regression test: events must be sent BEFORE receive() blocks.

    This verifies that the handle_io task runs concurrently and flushes writes.
    """
    test_program = """
use io::print
use messaging::receive

fn main(): () {
    print("Event 1: Before receive")
    let input = receive()
    print("Event 2: After receive")
}
"""

    with tempfile.NamedTemporaryFile(mode='w', suffix='.sa', delete=False) as f:
        f.write(test_program)
        temp_file = f.name

    try:
        collector = EventCollector()
        project_root = Path(__file__).parent.parent.parent

        async with acp.spawn_agent_process(
            lambda agent: collector,
            str(binary_path),
            "acp",
            "--engine", "print",
            "--file", temp_file,
            "--with-default-functions",
            "--with-acp-functions",
            cwd=project_root,
            transport_kwargs={"stderr": None},
            env={"RUST_LOG": "debug"},
        ) as (conn, process):
            await conn.initialize(
                protocol_version=acp.PROTOCOL_VERSION,
                client_info=acp.schema.Implementation(name="test", version="1.0")
            )

            session = await conn.new_session(cwd=str(project_root), mcp_servers=[])

            await asyncio.wait_for(collector.event1_received.wait(), timeout=20)

            collector.prompt_sent = True
            await conn.prompt(
                prompt=[acp.text_block("test")],
                session_id=session.session_id
            )

            await asyncio.sleep(0.5)

            assert any("Event 1" in e for e in collector.events_before_prompt), \
                f"Event 1 should arrive BEFORE prompt. Got: {collector.events_before_prompt}"

            assert any("Event 2" in e for e in collector.events_after_prompt), \
                f"Event 2 should arrive AFTER prompt. Got: {collector.events_after_prompt}"

    finally:
        Path(temp_file).unlink()
