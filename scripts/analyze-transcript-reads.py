import re
import sys
from collections import Counter
from pathlib import Path

FILE_PATH_RE = re.compile(
    r'`([^`]+\.[a-zA-Z0-9_]+)`'
    r'|"([^"]+\.[a-zA-Z0-9_/]+)"'
    r"|'([^']+\.[a-zA-Z0-9_/]+)'"
    r'|\b((?:[\w.-]+/)*[\w.-]+\.(?:rs|toml|md|py|json|sh|js|ts|txt|yaml|yml|lock))\b'
)

NOISE = re.compile(r'^https?://')
TOOL_CALL_RE = re.compile(r'\*\*Tool Call: agent::read_file\*\*')


def extract_paths_from_text(text: str) -> list[str]:
    paths = []
    for m in FILE_PATH_RE.finditer(text):
        candidate = next(g for g in m.groups() if g is not None)
        if NOISE.match(candidate):
            continue
        if len(candidate) > 200:
            continue
        paths.append(candidate)
    return paths


def analyse(transcript_path: Path) -> None:
    text = transcript_path.read_text()

    blocks = TOOL_CALL_RE.split(text)

    all_paths: list[str] = []
    for i, block in enumerate(blocks[1:], start=1):
        window = blocks[i - 1][-2000:]
        found = extract_paths_from_text(window)
        all_paths.extend(found)

    counts = Counter(all_paths)

    print(f"Transcript: {transcript_path}")
    print(f"Total read_file calls: {len(blocks) - 1}")
    print(f"Unique paths mentioned: {len(counts)}\n")
    print(f"{'Count':>6}  Path")
    print("-" * 50)
    for path, count in counts.most_common():
        print(f"{count:>6}  {path}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        default = Path(__file__).parent.parent / "docs" / "agent-long-task-transscript.md"
        target = default
    else:
        target = Path(sys.argv[1])

    if not target.exists():
        print(f"File not found: {target}", file=sys.stderr)
        sys.exit(1)

    analyse(target)
