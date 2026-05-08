import asyncio
import subprocess
import tempfile
from pathlib import Path

import mcp.types
import pytest
from fastmcp import Client
from fastmcp.client.transports import StdioTransport
from mcp.shared.exceptions import McpError

PROJECT_ROOT = Path(__file__).parent.parent.parent


def build_binary():
    result = subprocess.run(
        ["cargo", "build", "--bin", "structured-agent-workspace"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Failed to build binary: {result.stderr}")
    return PROJECT_ROOT / "target" / "debug" / "structured-agent-workspace"


@pytest.fixture(scope="session")
def binary_path():
    return build_binary()


def make_client(binary_path: Path, workspace_dir: str, timeout: float = 10.0) -> Client:
    root = mcp.types.Root(uri=f"file://{workspace_dir}", name="workspace")
    transport = StdioTransport(str(binary_path), [], cwd=str(PROJECT_ROOT))
    return Client(transport, roots=[root], timeout=timeout)


PYTHON_SOURCE = '''\
def greet(name: str) -> str:
    return f"Hello, {name}"

class Greeter:
    def __init__(self):
        pass
'''

RUST_SOURCE = '''\
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub struct Counter {
    value: i32,
}
'''


@pytest.fixture()
def workspace(tmp_path):
    (tmp_path / "hello.py").write_text(PYTHON_SOURCE)
    (tmp_path / "lib.rs").write_text(RUST_SOURCE)
    (tmp_path / "subdir").mkdir()
    (tmp_path / "subdir" / "nested.py").write_text("def nested(): pass\n")
    return tmp_path


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_list_directory_root(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        result = await client.call_tool("list_directory", {"path": "."})

    assert not result.is_error
    text = result.content[0].text
    assert "hello.py" in text
    assert "lib.rs" in text
    assert "subdir/" in text


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_list_directory_subdirectory(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        result = await client.call_tool("list_directory", {"path": "subdir"})

    assert not result.is_error
    assert "nested.py" in result.content[0].text


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_list_directory_nonexistent_returns_error(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        with pytest.raises(McpError, match="does not exist"):
            await client.call_tool("list_directory", {"path": "missing"})


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_list_directory_path_traversal_returns_error(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        with pytest.raises(McpError, match="escapes workspace root"):
            await client.call_tool("list_directory", {"path": "../"})


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_python_outline(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        result = await client.call_tool("read_file", {"path": "hello.py"})

    assert not result.is_error
    text = result.content[0].text
    assert "greet" in text
    assert "Greeter" in text


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_rust_outline(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        result = await client.call_tool("read_file", {"path": "lib.rs"})

    assert not result.is_error
    text = result.content[0].text
    assert "add" in text
    assert "Counter" in text


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_with_symbol_returns_definition(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        result = await client.call_tool("read_file", {"path": "hello.py", "symbol": "greet"})

    assert not result.is_error
    text = result.content[0].text
    assert "def greet" in text
    assert "Hello" in text


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_nonexistent_returns_error(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        with pytest.raises(McpError, match="does not exist"):
            await client.call_tool("read_file", {"path": "missing.py"})


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_unsupported_extension_returns_error(binary_path, workspace):
    (workspace / "data.csv").write_text("a,b,c\n")
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        with pytest.raises(McpError, match="Unsupported file type"):
            await client.call_tool("read_file", {"path": "data.csv"})


@pytest.mark.asyncio
@pytest.mark.timeout(30)
async def test_read_file_missing_symbol_returns_error(binary_path, workspace):
    async with make_client(binary_path, str(workspace)) as client:
        await asyncio.sleep(0.3)
        with pytest.raises(McpError, match="not found"):
            await client.call_tool("read_file", {"path": "hello.py", "symbol": "nonexistent"})
