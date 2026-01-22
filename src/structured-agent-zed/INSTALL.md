# Installation Guide for Structured Agent Zed Extension

This guide walks through installing the Structured Agent language extension for the Zed editor.

## Prerequisites

- Zed editor installed
- Node.js and npm (for building the tree-sitter grammar)
- Rust toolchain (for building the grammar bindings)
- tree-sitter CLI (will be installed via npm)

## Quick Installation

Run the automated setup script:

```bash
cd scratch/src/structured-agent-zed
./setup.sh
```

Then follow the instructions printed at the end to link the extension to Zed.

## Manual Installation

### Step 1: Build the Tree-sitter Grammar

Navigate to the tree-sitter grammar directory:

```bash
cd scratch/src/structured-agent-zed/tree-sitter-structured-agent
```

Install dependencies:

```bash
npm install
```

Generate the parser from the grammar:

```bash
npx tree-sitter generate
```

Test the grammar (optional but recommended):

```bash
npx tree-sitter test
```

### Step 2: Build Rust Bindings

Build the Rust bindings for the grammar:

```bash
cargo build --release
```

### Step 3: Install Extension in Zed

Create the Zed extensions directory if it doesn't exist:

```bash
mkdir -p ~/.config/zed/extensions
```

Link the extension directory (recommended for development):

```bash
ln -s /absolute/path/to/scratch/src/structured-agent-zed ~/.config/zed/extensions/structured-agent
```

Or copy the extension (for production use):

```bash
cp -r /path/to/scratch/src/structured-agent-zed ~/.config/zed/extensions/structured-agent
```

### Step 4: Restart Zed

Completely quit and restart Zed for the extension to be loaded.

## Verifying Installation

Open a `.sa` file in Zed (or create a new one). You should see:

- Syntax highlighting for keywords, types, strings, and comments
- Code outline in the sidebar showing function definitions
- Auto-indentation working inside blocks
- Comment toggling with `Cmd+/` (Mac) or `Ctrl+/` (Linux/Windows)

Try opening the included test file:

```bash
zed scratch/src/structured-agent-zed/test.sa
```

## Troubleshooting

### Extension Not Loading

Check that the extension directory exists:

```bash
ls -la ~/.config/zed/extensions/structured-agent
```

Verify the structure includes:

- `extension.toml`
- `languages/structured-agent/config.toml`
- `languages/structured-agent/highlights.scm`
- `tree-sitter-structured-agent/` directory

### Grammar Not Found

If you see errors about the grammar not being found, ensure:

1. The tree-sitter parser was generated (`parser.c` should exist in `tree-sitter-structured-agent/src/`)
2. The `extension.toml` correctly points to the grammar with `file://./tree-sitter-structured-agent`
3. You've restarted Zed after making changes

### Syntax Highlighting Not Working

Check the query files exist:

```bash
ls scratch/src/structured-agent-zed/languages/structured-agent/*.scm
```

You should see:
- `highlights.scm`
- `brackets.scm`
- `outline.scm`
- `indents.scm`

## Development Workflow

When developing the extension:

1. Make changes to grammar or query files
2. Regenerate the parser: `cd tree-sitter-structured-agent && npx tree-sitter generate`
3. Rebuild if needed: `cargo build --release`
4. Restart Zed to see changes

The `file://` URL in `extension.toml` means Zed loads the grammar from your local development directory, making iteration fast.

## Uninstalling

Remove the extension directory:

```bash
rm -rf ~/.config/zed/extensions/structured-agent
```

Or if you used a symlink:

```bash
unlink ~/.config/zed/extensions/structured-agent
```

Then restart Zed.

## Platform-Specific Notes

### macOS

Extension directory: `~/.config/zed/extensions`

### Linux

Extension directory: `~/.config/zed/extensions`

### Windows

Extension directory: `%APPDATA%\Zed\extensions`

Use backslashes in paths and adjust commands accordingly.

## Getting Help

If you encounter issues:

1. Check Zed's extension logs (if available)
2. Verify the grammar compiles: `npx tree-sitter generate`
3. Test the grammar: `npx tree-sitter test`
4. Ensure file permissions are correct on the extension directory
5. Try a clean reinstall by removing and re-linking the extension

## Next Steps

After installation, try:

- Opening existing `.sa` files from `scratch/src/structured-agent/samples/`
- Creating new Structured Agent programs
- Exploring the language features with syntax highlighting
- Using the code outline to navigate between functions