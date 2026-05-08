#!/usr/bin/env bash

greet() {
    local name="$1"
    echo "Hello, $name!"
}

function add() {
    echo $(( $1 + $2 ))
}

function main() {
    greet "World"
    add 1 2
}

main "$@"
