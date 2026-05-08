import re
import sys
from pathlib import Path

TOOL_CALL_RE = re.compile(r'^\*\*Tool Call: (.+?)\*\*$')


def parse_tool_calls(path: Path) -> list[tuple[int, str]]:
    calls = []
    with path.open() as f:
        for lineno, line in enumerate(f, 1):
            m = TOOL_CALL_RE.match(line.strip())
            if m:
                calls.append((lineno, m.group(1)))
    return calls


def build_tree(calls: list[tuple[int, str]]) -> None:
    CYCLE_END = "agent::write_file"
    SUB_TASKS_ENTER = "agent::plan_sub_tasks"
    SUB_TASK_DONE = "agent::summarize_sub_task"
    LOOP_ITER = "agent::has_repeated_last_action"
    BAILOUT = "agent::summarize_repeated_action"

    cycles: list[dict] = []
    current: dict = _new_cycle(0)

    for lineno, name in calls:
        current["all"].append((lineno, name))

        if name == LOOP_ITER:
            if current["sub_tasks_stack"]:
                current["sub_tasks_stack"][-1]["iters"] += 1
            else:
                current["top_iters"] += 1

        elif name == BAILOUT:
            if current["sub_tasks_stack"]:
                current["sub_tasks_stack"][-1]["bailouts"] += 1
            else:
                current["top_bailouts"] += 1

        elif name == SUB_TASKS_ENTER:
            frame = {"start": lineno, "sub_tasks": [], "iters": 0, "bailouts": 0, "current_sub": None}
            current["sub_tasks_stack"].append(frame)
            current["sub_tasks_flat"].append(frame)

        elif name == SUB_TASK_DONE:
            if current["sub_tasks_stack"]:
                frame = current["sub_tasks_stack"][-1]
                frame["sub_tasks"].append({"iters": frame["iters"], "bailouts": frame["bailouts"], "end": lineno})
                frame["iters"] = 0
                frame["bailouts"] = 0

        elif name == CYCLE_END:
            while current["sub_tasks_stack"]:
                current["sub_tasks_stack"].pop()
            current["end"] = lineno
            cycles.append(current)
            current = _new_cycle(lineno)

    if current["all"]:
        current["end"] = current["all"][-1][0]
        current["incomplete"] = True
        cycles.append(current)

    _print_tree(cycles)


def _new_cycle(start: int) -> dict:
    return {
        "start": start,
        "end": 0,
        "top_iters": 0,
        "top_bailouts": 0,
        "sub_tasks_stack": [],
        "sub_tasks_flat": [],
        "all": [],
        "incomplete": False,
    }


def _print_tree(cycles: list[dict]) -> None:
    total_calls = sum(len(c["all"]) for c in cycles)
    total_iters = sum(c["top_iters"] + sum(s["iters"] for st in c["sub_tasks_flat"] for s in st["sub_tasks"]) for c in cycles)
    total_bailouts = sum(c["top_bailouts"] + sum(s["bailouts"] for st in c["sub_tasks_flat"] for s in st["sub_tasks"]) for c in cycles)

    print(f"Total tool calls : {total_calls}")
    print(f"Total agent_loop iterations : {total_iters}")
    print(f"Total repeated-action bailouts: {total_bailouts}")
    print()
    print("main()")
    print("└── while true")

    for i, cycle in enumerate(cycles, 1):
        tag = " [incomplete]" if cycle.get("incomplete") else ""
        is_last = i == len(cycles)
        branch = "└──" if is_last else "├──"
        print(f"    {branch} [cycle {i}: L{cycle['start']+1}–L{cycle['end']}]{tag}")

        lines = []

        if cycle["top_iters"] > 0:
            bailout_note = f", {cycle['top_bailouts']} bailouts" if cycle["top_bailouts"] else ""
            lines.append(f"agent_loop() ×{cycle['top_iters']} iterations{bailout_note}")

        for j, st_frame in enumerate(cycle["sub_tasks_flat"]):
            n = len(st_frame["sub_tasks"])
            lines.append(f"sub_tasks() [{n} sub-tasks, from L{st_frame['start']}]")
            for k, sub in enumerate(st_frame["sub_tasks"]):
                sub_branch = "└──" if k == n - 1 else "├──"
                bail = f" + bailout" if sub["bailouts"] else ""
                lines.append(f"  {sub_branch} sub_task #{k+1}: agent_loop ×{sub['iters']}{bail} → L{sub['end']}")

        if not lines:
            lines.append("(no agent_loop iterations recorded)")

        for k, line in enumerate(lines):
            is_last_line = k == len(lines) - 1
            connector = "    └──" if is_last_line else "    ├──"
            print(f"    {'   ' if is_last else '│  '}{connector} {line}")


def main() -> None:
    if len(sys.argv) < 2:
        default = Path(__file__).parent.parent / "docs" / "agent-long-task-transscript.md"
        path = default
    else:
        path = Path(sys.argv[1])

    if not path.exists():
        print(f"File not found: {path}", file=sys.stderr)
        sys.exit(1)

    calls = parse_tool_calls(path)
    build_tree(calls)


if __name__ == "__main__":
    main()
