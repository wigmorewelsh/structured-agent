from typing import Any

from fastmcp import FastMCP

mcp = FastMCP("Echo Server")


@mcp.tool
def echo(message: str) -> str:
    """Echo back the provided message exactly as received."""
    return message


@mcp.tool
def echo_with_prefix(message: str, prefix: str = "Echo: ") -> str:
    """Echo back the message with a configurable prefix."""
    return f"{prefix}{message}"


@mcp.tool
def echo_json(data: dict[str, Any]) -> dict[str, Any]:
    """Echo back JSON data exactly as received."""
    return data


if __name__ == "__main__":
    mcp.run()
