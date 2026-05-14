# Known Issues

## Language

### `else if` / `elif`

Chained conditions require deep nesting. There is no `else if` or `elif` syntactic form; each alternative branch must be written as a nested `if` inside an `else` block.

### No parallel execution primitive

There is no language construct for running independent work concurrently. The dispatch pattern (multiple agents working in parallel) must be approximated by sequential loops or by spawning subagents in the harness layer.

### `mod` declaration is no longer required

Module files must not include a `mod <name>` declaration. The module identity is derived from the filename. If a `mod <name>` declaration is present the parser expects a `{` block to follow and rejects the file.

### Function name matches module name

When a module file exports a `pub fn` whose name is identical to the module name, the type checker reports the function as unknown at call sites in importing modules. The workaround is to give the exported function a name that differs from the module name.

Example: a file `plan_doc_reviewer.sa` exporting `pub fn plan_doc_reviewer()` fails resolution; renaming the function to `pub fn plan_doc_review()` resolves it.

### Single quotes inside triple-quoted strings

Single-quoted substrings inside `'''...'''` blocks are parsed as character or string literals rather than plain text. This causes parse errors when the content contains apostrophes (e.g. `implementer's`) or single-quoted identifiers (e.g. `'DONE'`). The workaround is to rephrase to avoid single quotes.

### `return expr!` is not valid

The `!` injection operator cannot be applied to a `return` expression. `return "string"!` is a parse error. Injection and return must be separate statements if both are needed, or the `!` omitted if only the return value matters.

## Stdlib / Types

### `execute_command` returns stdout only

There is no way to inspect the exit code or stderr of a command deterministically. Whether a command succeeded must be inferred by the LLM from the stdout content, which is unreliable for commands that fail silently or write diagnostics to stderr.

### `DateTime.now()`

There is no stdlib function to get the current date or time. Code that needs timestamped filenames must resort to `execute_command` with a shell date call, which couples the program to shell availability and makes the result LLM-interpreted rather than structured.

### Missing `List` methods

The `List` type has no stdlib methods for `is_empty()`, `length()`, or `append()`. List handling relies on LLM inference rather than deterministic operations, making it impossible to branch on list state or build lists programmatically in a type-checked way.
