#!/usr/bin/env bash

set -e

echo "Setting up Structured Agent Zed Extension..."

cd tree-sitter-structured-agent

if [ ! -f "package.json" ]; then
    echo "Error: package.json not found"
    exit 1
fi

echo "Installing npm dependencies..."
npm install

echo "Generating tree-sitter parser..."
npx tree-sitter generate

echo "Testing grammar..."
npx tree-sitter test

cd ..

echo "Building Rust bindings..."
cd tree-sitter-structured-agent
cargo build --release

cd ..

echo ""
echo "Setup complete!"
echo ""
echo "To install the extension in Zed:"
echo "  1. Create the extensions directory:"
echo "     mkdir -p ~/.config/zed/extensions"
echo ""
echo "  2. Link this extension:"
echo "     ln -s $(pwd) ~/.config/zed/extensions/structured-agent"
echo ""
echo "  3. Restart Zed"
echo ""
echo "Or copy the extension manually:"
echo "  cp -r $(pwd) ~/.config/zed/extensions/structured-agent"
