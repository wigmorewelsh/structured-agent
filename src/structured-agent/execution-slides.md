---
title: Structured Agent Execution Flow
theme: metropolis
aspectratio: 169
fontsize: 10pt
header-includes:
  - \usepackage{fvextra}
  - \usepackage{xcolor}
  - \DefineVerbatimEnvironment{Highlighting}{Verbatim}{breaklines,commandchars=\\\{\},fontsize=\footnotesize}
  - \DefineVerbatimEnvironment{verbatim}{Verbatim}{breaklines,commandchars=\\\{\},fontsize=\footnotesize}
---

# Overview

Currently all AI agents take an unstructured approach to execution. The language model selects tools to execute based on the context, prompts and tools available. Reasoning models have been fine tuned on top of patterns like Chain of Thought and ReAct to optimize the reliability of using tools to solve tasks. 

But the context is still unstructured. Processes will differ between runs, tools may or may not be run. e.g tests may or may not be run. Prompt Injection attacks are possible from input data or tools.

---

# Overview

What follows is an experimental new language that interleaves calls to the LLM before and after structured procedure calls. The LLM can be used to populate missing params when calling procedures and for defining the procedure responses. Context is managed based on the call stack of the procedures, i.e each procedure may add more context to the context, this is then discarded when out of scope.

By moving the process out of the models complete control and back into a structured language the same processes can be applied repeatably, prompt injection attack opportunities reduced, and developers can define reusable processes (procedures). 

---

# Unstructured agents

Context is always added to, every user interaction, model response, tool call, tool response is added for the duration of the agent.

There are some compaction strategies, removing old messages, using the model to summarize the context.

---

# Structured agents

Language model is implicitly called before fn calls to populate params. And after functions return to create their response.

Context is managed by the function call chain. Calling functions can add content to the context, returning from a function drops the context from the context.

---

# Context

:::::::::::::: {.columns}
::: {.column width="50%"}

**Context accumulation:**
- Each function call adds to context
- Context flows down the call stack
- Returning from function drops its context
- Sub-agents can create isolated contexts

**Example flow:**
```rust
fn main(ctx: Context) -> () {
    "You are helpful"!  // Base context
    
    let result = analyze(ctx, code);
    // analyze's context is dropped here
}
```

:::
::: {.column width="50%"}

**Context contains:**
- All previous prompt injections (`!`)
- Variable values when injected
- Function call history
- Return values from LLM calls

**Key properties:**
- **Scoped**: Context is function-local
- **Cumulative**: Builds up during execution
- **Automatic**: No manual context management
- **Isolated**: New contexts don't inherit

:::
::::::::::::::

---

# Auto populating missing function params

:::::::::::::: {.columns}
::: {.column width="50%"}

**Function call with placeholders:**
```rust
let result = analyze_code(ctx, _);
```

**Available context:**
```
You are a code analysis expert
The code to analyze: fn div(a, b) = a/b
```

:::
::: {.column width="50%"}

**LLM populates missing parameter:**

- Function signature: `analyze_code(ctx: Context, code: String)`
- LLM sees context and determines missing `code` parameter
- Automatically extracts: `"fn div(a, b) = a/b"`
- Function called as: `analyze_code(ctx, "fn div(a, b) = a/b")`

:::
::::::::::::::

---

# Function responses

:::::::::::::: {.columns}
::: {.column width="50%"}

**Function with prompts:**
```rust
fn analyze_code(ctx: Context, code: String) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}
```

**When called:**
- Prompts are injected into context
- LLM generates response based on context
- Return value is LLM's response

:::
::: {.column width="50%"}

**Response generation:**

1. **Context built:** All prompts + parameters
2. **LLM called:** Generates structured response
3. **Type conversion:** Response parsed to return type
4. **Function returns:** Typed result to caller

:::
::::::::::::::

---

# Function responses - explicit return

:::::::::::::: {.columns}
::: {.column width="50%"}

**Implicit LLM generation:**
```rust
fn analyze_code(ctx: Context, code: String) -> Analysis {
    "Analyze the following code for potential bugs"!
    code!
    // LLM generates response automatically
}
```

**Explicit return bypasses LLM:**
```rust
fn add(ctx: Context, x: i32, y: i32) -> i32 {
    "Adding two numbers"!
    return x + y  // Direct computation
}
```

:::
::: {.column width="50%"}

TODO

:::
::::::::::::::

---

# Worked Example

\vfill
\centering
\Large
**Worked Example**
\vfill

---

# main() execution outline

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(ctx: Context) -> () {
    let code = "fn div(a, b): return a / b";
    
    "You are a code analysis expert"!
    
    let analysis = ctx.analyze_code(code);
    
    let fix = ctx.suggest_fix(analysis);
}
```

:::
::: {.column width="50%"}

**Two LLM calls will be made:**

1. `analyze_code(code)` → LLM call
2. `suggest_fix(analysis)` → LLM call

:::
::::::::::::::

---

# Step 1: analyze_code function call

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn analyze_code(Context, code) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}
```

:::
::: {.column width="50%"}

**Sent to LLM:**
```
\textcolor{blue}{You are a code analysis expert}
Analyze the following code for potential bugs
Focus on edge cases and error handling
fn div(a, b): return a / b
```

:::
::::::::::::::

---

# Step 1 Result: LLM Response

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let analysis = ctx.analyze_code(code);
// Contains LLM result
```

:::
::: {.column width="50%"}

**LLM Returns:**
```
Division by zero error possible. Function lacks input validation and error handling for b=0 case.
```

:::
::::::::::::::

---

# Step 2: suggest_fix function call

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn suggest_fix(Context, analysis: Analysis) -> CodeFix {
    "Given this analysis, suggest a fix"!
    analysis!
}
```

:::
::: {.column width="50%"}

**Sent to LLM:**
```
\textcolor{blue}{You are a code analysis expert}
Given this analysis, suggest a fix
Division by zero error possible. Function lacks input validation and error handling for b=0 case.
```

:::
::::::::::::::



---

# Complete execution trace

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(ctx: Context) -> () {
    let code = "fn div(a, b): return a / b";
    
    "You are a code analysis expert"!
    
    let analysis = ctx.analyze_code(code);
    
    let fix = ctx.suggest_fix(analysis);
}
```

:::
::: {.column width="50%"}

**LLM Call 1:**
```
\textcolor{blue}{You are a code analysis expert}
Analyze the following code for potential bugs
Focus on edge cases and error handling
fn div(a, b): return a / b
```

**LLM Call 2:**
```
\textcolor{blue}{You are a code analysis expert}
Given this analysis, suggest a fix
Division by zero error possible. Function lacks input validation and error handling for b=0 case.
```

:::
::::::::::::::

---

# Select Statement Overview

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn calculator_agent(ctx: Context, request: String) -> i32 {
    "You are a calculator. Use the tools provided."!
    request!
    
    let result = select {
        case add(ctx, _, _):
            @results@
        case subtract(ctx, _, _):
            @results@
    }
    
    return result
}
```

:::
::: {.column width="50%"}

**Select statement allows LLM to:**

1. Choose which tool to execute
2. Provide parameters using `_` placeholders  
3. Access tool results via `@results@`

:::
::::::::::::::

---

# Select Statement: Tool Choice

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let result = select {
    case add(ctx, _, _):
        @results@
    case subtract(ctx, _, _):
        @results@
}
```

**User request:** "Calculate 2 - 5"

:::
::: {.column width="50%"}

**Sent to LLM:**
```
\textcolor{blue}{You are a calculator. Use the tools provided.}
Calculate 2 - 5

Choose tool and provide parameters:
- add(ctx, x, y)
- subtract(ctx, x, y)
```

**LLM chooses:** `subtract(ctx, 2, 5)`

:::
::::::::::::::

---

# Select Statement: Tool Execution

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn subtract(ctx: Context, x: i32, y: i32) -> i32 {
    x - y
}

case subtract(ctx, _, _):
    @results@
```

:::
::: {.column width="50%"}

**Execution flow:**

1. LLM selected: `subtract(ctx, 2, 5)`
2. Function executes: `2 - 5 = -3`
3. `@results@` contains: `-3`
4. Case handler returns: `-3`

:::
::::::::::::::

---



# Select Statement: Complete Flow

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn calculator_agent(ctx: Context, request: String) -> i32 {
    "You are a calculator. Use the tools provided."!
    request!
    
    let result = select {
        case add(ctx, _, _):
            @results@
        case subtract(ctx, _, _):
            @results@
    }
    
    return result
}
```

:::
::: {.column width="50%"}

**For "Calculate 2 - 5":**

1. **Context:** Calculator prompt + user request
2. **Tool Choice:** LLM selects `subtract(ctx, 2, 5)`
3. **Execution:** `subtract` returns `-3`
4. **Result:** `@results@` = `-3`
5. **Return:** Function returns `-3`

:::
::::::::::::::

---



# External Functions

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
extern fn add(x: i32, y: i32) -> i32;

extern fn subtract(x: i32, y: i32) -> i32;

fn calculator_agent(ctx: Context, request: String) -> i32 {
    "You are a calculator. Use the tools provided."!
    request!
    
      let result = select {
        add_, _) as a => { ...transform_a... },
        substract(_ _) as b => b
      }
    
    return result
}
```

:::
::: {.column width="50%"}

**External functions:**

- Declared with `extern` keyword
- Implemented outside the language
- Callable from `select` statements
- No context parameter needed
- Direct native execution

:::
::::::::::::::

---

# Sub-Agents with New Context

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn research_agent(ctx: Context, topic: String) -> String {
    "You are a research specialist"!
    topic!
    
    "Combine your knowledge with this analysis:"!
    
    let sub_context = Context::new()
    analysis_agent(sub_context, topic)!
}

fn analysis_agent(ctx: Context, topic: String) -> String {
    "You are an expert analyst"!
    topic!
}
```

:::
::: {.column width="50%"}

**Sub-agent execution:**

1. **Parent Context:** Research specialist prompt + topic
2. **New Context:** `Context::new()` creates fresh context
3. **Sub-agent:** `analysis_agent` runs with isolated context
4. **Context Isolation:** Sub-agent only sees "expert analyst" + topic
5. **Result Integration:** Sub-agent result added to parent context

:::
::::::::::::::


---

# Further

Its possible to implement Chain of Thought and ReAct on top of this language, with modifications. By using this language more reliable alternatives can be implemented with the same ease.

So then are fine-tuned more expensive reasoning models really needed? Do models really need to be so large, could smaller ones do?
