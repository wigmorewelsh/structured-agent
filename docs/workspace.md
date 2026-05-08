# Workspace

The workspace is an MCP server that mediates all file access for a coding agent. Rather than exposing raw file read and write primitives, it presents a structured view of the codebase: small files are returned in full, large files as a symbol outline, and individual symbols or line ranges on demand. Writes target symbols or whole files rather than arbitrary text ranges. This keeps the agent's context lean and makes edits precise.

The workspace is implemented in `src/structured-agent-workspace` and exposes its interface over stdio as an MCP tool server.

## Workspace Root

The workspace root — the directory the server is permitted to access — is provided by the MCP client during the `initialize` handshake via the standard `roots` capability. The server calls `peer.list_roots()` after initialization and uses the first root whose URI resolves to a local directory. The `--workspace` CLI flag is removed; there is nothing to configure at launch time.

If the client provides no roots, or none resolve to an accessible local directory, the server returns an error on every tool call rather than falling back silently.

## Supported Languages

Tree-sitter grammars drive symbol extraction. The following are supported:

- Rust (`.rs`)
- Python (`.py`)
- Markdown (`.md`)
- Shell (`.sh`, `.bash`)

Files with unsupported extensions are still accessible in full provided they are below the size threshold. They cannot be outlined or edited by symbol.

## Reading

Reading is the primary operation. The server adapts its response based on the file's size relative to a threshold (currently under consideration, likely in the region of 300–500 lines).

**Small files** are returned in full, verbatim, without any annotation. The agent receives the complete text and can reason about it directly.

**Large files** are returned as a symbol outline. Each entry in the outline gives the symbol's kind, qualified name, and line range. For Rust and Python this covers functions, structs, enums, traits, impl blocks, and type aliases. For Markdown it covers headings. Shell outlines cover function definitions. The format is `{start}-{end} {kind} {name}`, one symbol per line.

When the agent needs the body of a specific symbol it calls `read_file` again with the `symbol` parameter. The server extracts the full source text for that symbol using tree-sitter and returns it alone. If the symbol is a method on an impl block, it can be addressed as `TypeName::method_name`.

Line-range reads are also available for cases where the agent has identified a relevant region but there is no enclosing symbol — common in shell scripts and Markdown. The agent supplies `start_line` and `end_line` and receives those lines verbatim.

## Searching

Two search tools are planned.

**Symbol search** (`search_symbols`) will accept a name or partial name and return all matching symbols across all files in the workspace, along with the file path and line range. This is driven by tree-sitter outlines computed on demand and is the preferred way to locate a definition.

**Text search** (`search_text`) will accept a regex pattern and return matching lines with their file path and line number. It will respect `.gitignore`. This is the fallback for finding strings, comments, or patterns that do not correspond to named symbols.

Both tools will accept an optional glob pattern to restrict the search to a subtree.

## Editing

Editing is designed around the same symbol model as reading. Three edit primitives are provided.

**Symbol replacement** (`patch_symbol`) is the primary edit tool for code files. The agent supplies a file path, a symbol name, and the full replacement text for that symbol. The server locates the symbol using tree-sitter, replaces the exact byte range occupied by that symbol's definition, and writes the file back. The operation is stateless: the symbol is re-located from the current file contents on every call, so external changes to the file — formatting, manual edits, compiler output — do not invalidate anything. The trade-off is that if the symbol has been renamed externally the agent must use the new name.

**Full file write** (`write_file`) replaces the entire contents of a file. This is the appropriate tool for small files, generated files, and files that have no parseable symbols. It is also the safe fallback when symbol replacement is insufficient.

**Line-range replacement** (`patch_lines`) replaces a contiguous range of lines identified by `start_line` and `end_line`. This exists for non-code files and for edits inside a large symbol body where the agent wants to change only a portion. Because it is line-number based it is sensitive to the file having changed since the last read; the caller is expected to re-read the file if there is any doubt. Hash-anchored line editing, which removes this sensitivity, is on the longer-term plan and is discussed separately below.

## Longer-Term: Hash-Anchored Edits

The line-range replacement tool has an inherent fragility: line numbers shift when any preceding edit inserts or removes lines, and the agent has no way to know whether the file has changed on disk between a read and a write.

A hash-anchored edit scheme addresses this. Each line returned by a read carries a short prefix derived from its content — sufficient to identify it uniquely within the file without encoding a line number. An edit specifies anchor hashes rather than line numbers. The server validates the anchors against the current file contents before applying the edit; if the file has changed and the anchor no longer matches, it returns a clear error rather than applying the edit to the wrong location.

This is intentionally stateless. The anchor is derived from the line content, not stored in server memory, which means external file changes are handled naturally: unchanged lines still have valid anchors, changed lines do not. The design deliberately avoids stateful anchor registries, which require file-watching infrastructure and introduce silent failures when the watch misses an event.

The scheme is not implemented in the initial version. `patch_lines` is the interim tool, with the expectation that callers re-read before writing in any situation where the file may have changed.

## Sources

- Dirac hash-anchored edit design: https://dirac.run/posts/hash-anchors-myers-diff-single-token
- The Harness Problem (original hash-anchor idea): referenced in the above post, by Can Bülük
- MCP roots specification: https://spec.modelcontextprotocol.io/specification/2025-03-26/client/roots/
- rmcp `Peer::list_roots`: `src/structured-agent-workspace/src/workspace.rs`
