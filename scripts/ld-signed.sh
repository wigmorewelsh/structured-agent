#!/bin/sh
/usr/bin/cc -fuse-ld=/usr/local/opt/lld@20/bin/ld64.lld "$@"
CC_EXIT=$?

if [ $CC_EXIT -eq 0 ]; then
    OUTPUT=""
    NEXT_IS_OUTPUT=false

    for arg in "$@"; do
        if [ "$NEXT_IS_OUTPUT" = true ]; then
            OUTPUT="$arg"
            break
        fi
        if [ "$arg" = "-o" ]; then
            NEXT_IS_OUTPUT=true
        fi
    done

    if [ -n "$OUTPUT" ] && [ -f "$OUTPUT" ] && [ -x "$OUTPUT" ]; then
        codesign --force --sign - "$OUTPUT" 2>/dev/null || true
    fi
fi

exit $CC_EXIT
