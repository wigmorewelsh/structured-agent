#!/bin/sh
set -e

echo "Signing development toolchain binaries to prevent Gatekeeper CPU usage..."
echo "This script requires sudo access for Homebrew-installed tools."

RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

sign_path() {
    local path=$1
    local use_sudo=$2

    if [ ! -d "$path" ] && [ ! -f "$path" ]; then
        return 0
    fi

    echo "Signing: $path"

    if [ -f "$path" ]; then
        if [ "$use_sudo" = "true" ]; then
            sudo codesign --force --sign - "$path" 2>/dev/null || echo "  Warning: Failed to sign $path"
        else
            codesign --force --sign - "$path" 2>/dev/null || echo "  Warning: Failed to sign $path"
        fi
    else
        local count
        if [ "$use_sudo" = "true" ]; then
            count=$(sudo find "$path" -type f \( -perm +111 -o -name "*.dylib" -o -name "*.so" \) -exec codesign --force --sign - {} \; 2>/dev/null | wc -l | tr -d ' ')
        else
            count=$(find "$path" -type f \( -perm +111 -o -name "*.dylib" -o -name "*.so" \) -exec codesign --force --sign - {} \; 2>/dev/null | wc -l | tr -d ' ')
        fi
        echo "  Signed files in directory"
    fi
}

echo ""
echo "=== Rust Toolchains (rustup) ==="
if [ -d "$RUSTUP_HOME/toolchains" ]; then
    for toolchain in "$RUSTUP_HOME/toolchains"/*; do
        if [ -d "$toolchain" ] && [ ! -L "$toolchain" ]; then
            toolchain_name=$(basename "$toolchain")
            echo "Processing toolchain: $toolchain_name"
            sign_path "$toolchain" false
        fi
    done
else
    echo "No rustup toolchains found at $RUSTUP_HOME/toolchains"
fi

echo ""
echo "=== Cargo User Binaries ==="
if [ -d "$CARGO_HOME/bin" ]; then
    sign_path "$CARGO_HOME/bin" false
else
    echo "No cargo bin directory found at $CARGO_HOME/bin"
fi

echo ""
echo "=== Homebrew-installed Tools ==="

sign_homebrew_tool() {
    local name=$1
    local pattern=$2

    if [ -z "$pattern" ]; then
        pattern="$name"
    fi

    local cellar_path=$(find /usr/local/Cellar/"$pattern"* -maxdepth 0 -type d 2>/dev/null | head -1)
    if [ -n "$cellar_path" ] && [ -d "$cellar_path" ]; then
        echo "$name: $cellar_path"
        sign_path "$cellar_path" true
    else
        echo "$name: not found"
    fi
}

sign_homebrew_tool "Rust" "rust"
sign_homebrew_tool "rustup" "rustup"
sign_homebrew_tool "cargo-nextest" "cargo-nextest"
sign_homebrew_tool "cargo-instruments" "cargo-instruments"
sign_homebrew_tool "sccache" "sccache"
sign_homebrew_tool "UV" "uv"
sign_homebrew_tool "Node.js" "node"

echo ""
echo "=== LLVM and LLD ==="
if [ -d "/usr/local/opt/llvm" ]; then
    sign_path "/usr/local/opt/llvm/bin" true
    sign_path "/usr/local/opt/llvm/lib" true
else
    echo "LLVM: not found"
fi

if [ -d "/usr/local/opt/lld@20" ]; then
    sign_path "/usr/local/opt/lld@20/bin" true
else
    echo "LLD: not found"
fi

echo ""
echo "=== Python ==="
for python_version in /usr/local/opt/python@* /usr/local/Cellar/python@*/*/Frameworks/Python.framework/Versions/*; do
    if [ -d "$python_version" ]; then
        echo "Python: $python_version"
        sign_path "$python_version" true
        break
    fi
done

echo ""
echo "Done! All development toolchain binaries have been signed."
echo ""
echo "Note: Run this script again after:"
echo "  - rustup update"
echo "  - rustup toolchain install <version>"
echo "  - brew upgrade (for any of the signed tools)"
echo "  - Installing new cargo binaries with 'cargo install'"
