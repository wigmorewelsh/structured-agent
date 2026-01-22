# Structured Agent Language Extension for Zed

Language support for the Structured Agent language in the Zed editor.

## Features

- Syntax highlighting for `.sa` files
- Bracket matching
- Code outline
- Auto-indentation
- Comment toggling with `#`

## Installation

### Local Development Setup

The extension is configured to use a local file path for the tree-sitter grammar, making it easy to develop and test locally.

1. Generate the tree-sitter parser:

```bash
cd tree-sitter-structured-agent
npm install
npx tree-sitter generate
```

2. The `extension.toml` uses `file://./tree-sitter-structured-agent` which references the local grammar directory.

3. Link or copy the extension to Zed's extensions directory:

```bash
mkdir -p ~/.config/zed/extensions
ln -s /absolute/path/to/structured-agent-zed ~/.config/zed/extensions/structured-agent
```

Or use the setup script:

```bash
./setup.sh
```

4. Restart Zed

The extension will now load the grammar from your local development directory, allowing you to iterate on both the grammar and highlighting rules.

## Structured Agent Language

The Structured Agent language is a hybrid system where developers define process structure while models handle reasoning and decision-making within that structure.

### Syntax Highlights

- **Keywords**: `fn`, `extern`, `let`, `if`, `while`, `return`, `select`, `as`
- **Types**: `String`, `Boolean`, `i32`, `Context`, `()`
- **Operators**: Context injection `!`, assignment `=`, arrow `=>`
- **Comments**: Line comments starting with `#`

### Example

```structured-agent
fn greet(ctx: Context, name: String) -> String {
    "Generate a friendly greeting for "!
    name!
}

fn main() -> () {
    "Starting program"!
    let result = greet(ctx, "Alice")
    result!
}
```

## Development

### Building the Grammar

The tree-sitter grammar is defined in `tree-sitter-structured-agent/grammar.js`. After making changes:

```bash
cd tree-sitter-structured-agent
npx tree-sitter generate
npx tree-sitter test
```

Since the extension uses `file://./tree-sitter-structured-agent`, changes to the grammar will be picked up when you regenerate the parser and restart Zed.

### Query Files

Syntax highlighting and editor features are controlled by tree-sitter query files in `languages/structured-agent/`:

- `highlights.scm` - Syntax highlighting
- `brackets.scm` - Bracket matching
- `outline.scm` - Code outline structure
- `indents.scm` - Auto-indentation rules

## Language Reference

The Structured Agent language documentation can be found in the main project repository.

## License

MIT