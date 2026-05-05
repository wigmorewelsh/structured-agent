---
title: Structured Agent — Language Overview
theme: metropolis
aspectratio: 169
fontsize: 10pt
header-includes:
  - \usepackage{fvextra}
  - \usepackage{xcolor}
  - \DefineVerbatimEnvironment{Highlighting}{Verbatim}{breaklines,commandchars=\\\{\},fontsize=\footnotesize}
  - \DefineVerbatimEnvironment{verbatim}{Verbatim}{breaklines,commandchars=\\\{\},fontsize=\footnotesize}
---

# What it is

An experimental language that interleaves LLM calls with deterministic procedure calls.

- The programmer defines the process
- The LLM fills gaps — parameters, typed responses, decisions
- Context is managed per call scope — not by the programmer

---

# Function Syntax

```rust
fn analyze(code: String): Analysis {
    "Analyze this code"!
    code!
}

fn main(): () {
    let result = analyze(code)
}
```

- No `ctx: Context` parameter — context is implicit, scoped to the call
- Return type uses `:`
- Functions with prompt injections return LLM-generated values

---

# Implicit Context

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn analyze(code: String): Analysis {
    "You are a code reviewer"!
    code!
}

fn main(): () {
    "You are a senior engineer"!
    let result = analyze(code)
    // analyze's context dropped here
}
```

:::
::: {.column width="50%"}

**LLM call 1 sees:**
```
You are a senior engineer
You are a code reviewer
<code>
```

**After return:**
```
You are a senior engineer
```

:::
::::::::::::::

---

# Structs

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
struct Address {
    city: String,
}

struct Person {
    name: String,
    address: Address,
}

fn main(): String {
    let p = Person {
        name: "Alice",
        address: Address { city: "London" }
    }
    return p.address.city
}
```

:::
::: {.column width="50%"}

- Declared with `struct`
- Nested structs
- Field access with `.`
- Can be injected: `r!`
- Can be LLM return types

:::
::::::::::::::

---

# Generics

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
struct Wrapper<T> {
    label: String,
    value: T,
}

struct Profile {
    name: String,
    bio: Option<String>,
}

fn main(): List<Task> {
    return [
        Task { title: "write tests", done: true },
        Task { title: "fix bugs",    done: false }
    ]
}
```

:::
::: {.column width="50%"}

- `struct Name<T>` — generic structs
- `Option<T>` — nullable fields
- `List<T>` — typed list literals
- Type parameters guide the LLM's JSON response schema

:::
::::::::::::::

---

# Module System

:::::::::::::: {.columns}
::: {.column width="50%"}

**`greeter.sa`**
```rust
mod greeter

pub fn greet(name: String): String {
    return name
}
```

**`main.sa`**
```rust
use greeter::greet

fn main(): () {
    let result = greet("Alice")
    result!
}
```

:::
::: {.column width="50%"}

- `mod name` declares the module
- `pub fn` exports a function
- `use module::fn` imports it
- Files discovered from the working directory

:::
::::::::::::::

---

# Module Interfaces: `sig`

:::::::::::::: {.columns}
::: {.column width="50%"}

**`formatter.sa`**
```rust
mod formatter

sig Formatter {
    fn format(value: String): String
}

pub fn format(value: String): String {
    return value
}
```

**`reporter.sa`**
```rust
mod reporter(fmt: formatter::Formatter)

pub fn report(value: String): () {
    let formatted = fmt::format(value)
    formatted!
}
```

:::
::: {.column width="50%"}

**`main.sa`**
```rust
mod reporter(formatter)

use reporter::report

fn main(): () {
    report("hello")
}
```

- `sig` declares a structural interface
- Modules can take other modules as parameters
- Dispatch is vtable-based at runtime
- Dependency injection without an object system

:::
::::::::::::::

---

# Actors: Declaration

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
struct State {}

mod Worker(state: State) {
    pub fn run(): () {
        "Starting work"!
        while true {
            "Next step"!
            yield
        }
    }

    pub fn status(): String {
        "Current status"!
    }
}
```

:::
::: {.column width="50%"}

- `mod Name(state)` — actor module with state
- `pub fn` — methods callable from outside
- `yield` — suspends the actor, lets other work proceed
- Each actor has its own LLM context

:::
::::::::::::::

---

# Actors: Spawning and Calling

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(): String {
    let w = spawn<Worker>("w1", State{})
    defer w::run()
    let s = w::status()
    return s
}
```

:::
::: {.column width="50%"}

- `spawn<Name>("id", state)` — creates an instance
- `w::fn()` — calls a method on the actor
- `defer w::fn()` — starts running concurrently, suspends at `yield`
- `w::status()` runs to completion while `run` is suspended

:::
::::::::::::::

---

# Select Statement

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let result = select {
    add(_, _),
    subtract(_, _)
}
```

:::
::: {.column width="50%"}

- LLM sees the accumulated context and the list of functions
- Picks one branch and fills any `_` parameters
- The result of the chosen branch is the value of the expression

:::
::::::::::::::

---

# Select: Step by Step

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn agent_loop(): String {
    "what is the next action?"!

    let result = select {
        read_file(_),
        write_file(_, _),
        list_directory(_)
    }

    result!

    if is_task_complete() {
        return summarize()
    } else {
        return agent_loop()
    }
}
```

:::
::: {.column width="50%"}

**For: "add SUCCESS to test.tmp"**

1. LLM selects `read_file(_)`, fills `"test.tmp"`
2. Contents returned, injected via `result!`
3. `is_task_complete()` → false
4. Recurse — context now includes file contents
5. LLM selects `write_file(_, _)`, fills path + new content
6. `is_task_complete()` → true → `summarize()`

:::
::::::::::::::

---

# Standard Library

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
use io::print
use io::input
use messaging::receive
use unstable::head
```

:::
::: {.column width="50%"}

| Module | Functions |
|---|---|
| `io` | `print`, `input` |
| `messaging` | `receive` |
| `fs` | file system ops |
| `math` | arithmetic |
| `logic` | boolean helpers |
| `equality` | equality checks |
| `unstable` | `head`, `tail`, option helpers |
| `actor` | actor system support |

:::
::::::::::::::

---

# LLM Backends

:::::::::::::: {.columns}
::: {.column width="50%"}

```
# Gemini
sa run --engine gemini
       --gemini-api-key KEY
       --gemini-model gemini-2.5-pro

# OpenAI
sa run --engine openai
       --openai-api-key KEY
       --openai-model gpt-4o

# Hugging Face (OpenAI-compatible)
sa run --engine huggingface
       --hf-token TOKEN
       --hf-model Qwen/Qwen2.5-72B-Instruct
```

:::
::: {.column width="50%"}

- All three backends implement the same `LanguageEngine` trait
- Same `.sa` program runs unchanged against any backend
- HuggingFace reuses the OpenAI engine with a different base URL
- Config can live in `config.toml` instead of CLI flags

:::
::::::::::::::

---

# LLM Thinking (Gemini)

:::::::::::::: {.columns}
::: {.column width="50%"}

| Level | Budget |
|---|---|
| `disabled` | 0 tokens |
| `low` | 512 tokens |
| `medium` | model-determined |
| `high` | model-determined |
| `with_budget(n)` | explicit |

:::
::: {.column width="50%"}

- Configured on the engine before the runtime starts
- The model reasons internally before generating each response
- Thoughts are stored as `ThinkingEvent` in context
- Fed back to the model on subsequent turns alongside regular context

:::
::::::::::::::

---

# Worked Example

\vfill
\centering
\Large
**A full agent**
\vfill

---

# Agent: Structure

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(): () {
    '''
    You are a structured agent.
    Use print to respond to users.
    '''!

    let history = read_file("history.md")

    while true {
        history!
        let task = receive()
        task!
        plan()!
        let summary = agent_loop()
        history = update_history(summary)
        write_file("history.md", history)
    }
}
```

:::
::: {.column width="50%"}

```rust
fn agent_loop(): String {
    "what is the next action?"!

    let result = select {
        plan(),
        edit_file(_),
        read_file(_),
        write_file_content(_, _),
        list_directory(_),
        grep(_, _, _, _, _),
        execute_command(_),
        ask_for_more_info(_)
    }

    result!

    if is_task_complete() {
        return summarize()
    } else {
        return agent_loop()
    }
}
```

:::
::::::::::::::

---

# Agent: What the Structure Enforces

- The outer loop **always** reads history, receives a task, plans, loops, then writes history
- The inner loop **always** checks `is_task_complete()` before stopping
- The model **cannot** skip planning, skip writing history, or exit early
- Prompt injection through tool results passes through `check_content()` — enforced by the language, not the model

---

# Summary

:::::::::::::: {.columns}
::: {.column width="50%"}

**Language features added since v1:**

- Implicit context (no `ctx` param)
- Structs with generics
- `Option<T>`, `List<T>`
- Full module system
- `sig` interfaces
- Actors with `yield` and `defer`
- Simplified `select`
- Standard library
- Multi-line `'''...'''` strings

:::
::: {.column width="50%"}

**Runtime features:**

- Gemini, OpenAI, Hugging Face backends
- LLM thinking support
- Struct schema inference for typed LLM responses
- Actor registry and scheduler

**The key question remains:** how small can the model be before structured execution makes up the difference?

:::
::::::::::::::