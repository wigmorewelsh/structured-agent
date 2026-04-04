# Bytecode Architecture

The bytecode instruction set provides a linear, stack-based intermediate representation for the agent runtime. This architecture enables execution state serialization, supporting pause and resume capabilities essential for durable agent execution.

## Instruction Set

The bytecode uses a stack-based execution model where operands are pushed onto an evaluation stack and consumed by operations. Each instruction advances the program counter unless it performs an explicit jump.

### Stack Operations

The `push.const` instruction loads a constant from the constant pool onto the stack. Constants are referenced by index and include primitives like strings, booleans, and unit values.

The `push.var` instruction retrieves a variable's value from the current context and pushes it onto the stack. Variable resolution follows the context chain, respecting scope boundaries.

The `pop` instruction removes the top value from the stack without storing it. The `dup` instruction duplicates the top stack value.

### Variable Operations

The `declare.var` instruction pops the top stack value and binds it to a new variable in the current context. This creates a new binding that shadows any parent scope variable with the same name.

The `store.var` instruction pops the stack and assigns the value to an existing variable, traversing parent contexts until it finds the declaration. An error occurs if no such variable exists.

### Control Flow

The `call` instruction invokes a function with arguments taken from the stack. The number of arguments is specified in the instruction. Before jumping to the function's first instruction, a stack frame is pushed containing the return address and context reference.

The `ret` instruction returns from the current function by popping the stack frame and restoring the previous program counter and context.

Branch instructions control conditional execution. The `br` instruction performs an unconditional jump to the specified offset. The `br.false` and `br.true` instructions pop a boolean from the stack and jump only if the condition matches.

### List Operations

The `make.list` instruction pops N elements from the stack and constructs a list value containing those elements in reverse order (since stack operations are LIFO).

### Select Operations

Select expressions enable concurrent evaluation patterns. The `select.begin` instruction marks the start of a select block. Each `select.clause` instruction binds the result of an expression to a variable and specifies the offset to the continuation code. The `select.end` instruction marks the completion of a select branch.

### Scope Operations

The `enter.scope` instruction creates a new context as a child of the current context without creating a scope boundary. The `exit.scope` instruction returns to the parent context.

### Context Operations

The `push.context` instruction creates a new context with a scope boundary, used when entering function bodies. The `pop.context` instruction restores the parent context.

### VM Control

The `yield` instruction marks a point where execution can pause. The VM saves the current execution state and returns control to the caller. Execution resumes from the instruction following the yield.

The `halt` instruction terminates execution.

## Compilation Examples

A variable declaration compiles to a constant push followed by a declare operation:

```
let x = "hello"

    push.const 0        ; "hello"
    declare.var "x"
```

Function calls push arguments in order before invoking the function:

```
add(x, "world")

    push.var "x"
    push.const 0        ; "world"
    call add 2
```

Conditional statements use branch instructions with labels marking jump targets:

```
if (condition) { body } else { else_body }

L0: push.var "condition"
    br.false L1
    enter.scope
    ; body bytecode
    exit.scope
    br L2
L1: enter.scope
    ; else_body bytecode
    exit.scope
L2: ; continue
```

Loops use backward jumps to implement iteration:

```
while (condition) { body }

L0: push.var "condition"
    br.false L2
    enter.scope
    ; body bytecode
    exit.scope
    br L0
L2: ; continue
```

## Execution State

The VM maintains execution state comprising the program counter, evaluation stack, call stack, and current context reference. This state is fully serializable.

The program counter holds the offset of the next instruction to execute within the current function. The evaluation stack holds intermediate values during expression evaluation. The call stack contains frames for each active function call, where each frame records the function name, return address, base pointer, and context identifier.

## Serialization Format

The bytecode and execution state use a serializable format enabling persistence to the graph database. Instructions encode as enumerated types with operands. The constant pool stores literal values referenced by instructions. Function definitions bundle the instruction sequence, constant pool, and parameter names.

An execution snapshot captures the program counter, current function name, evaluation stack contents, call stack frames, and context identifier. This snapshot, combined with the bytecode program and context graph state, provides complete restoration of execution.

## Native Function Interface

Native functions implemented in Rust integrate through the `call.native` instruction. The VM invokes the native implementation through the Expression trait, passing the current context. Native functions return results that the VM pushes onto the stack.

This boundary separates interpreted bytecode execution from native operations like HTTP requests, file system access, and external API calls. Native functions may be asynchronous, with yield points marking await boundaries.

## Performance Characteristics

Stack-based bytecode offers compact representation and simple dispatch logic. Each instruction performs a single operation, making the interpreter loop straightforward. The linear instruction sequence enables efficient caching and prefetching.

The architecture supports future optimization through just-in-time compilation. The bytecode serves as an intermediate representation that a backend like Cranelift can compile to native code, with the interpreter providing a fallback for cold paths.

## Design Rationale

The stack-based model simplifies compilation from the abstract syntax tree. Expression evaluation naturally produces stack operations, and control flow analysis maps directly to branch instructions. The instruction set remains minimal, with complex operations delegated to function calls.

Explicit yield points enable cooperative multitasking without requiring preemptive thread scheduling. The VM controls when execution pauses, ensuring consistent state at serialization points.

The separation between bytecode, execution state, and context graph allows independent versioning. Bytecode remains immutable and version-controlled. Execution snapshots reference specific bytecode versions, supporting replay and debugging of historical execution.