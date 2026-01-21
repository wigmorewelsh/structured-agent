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
        add(ctx, _, _) as r => r,
        subtract(ctx, _, _) as r => r
    }
    
    return result
}
```

:::
::: {.column width="50%"}

**Select statement allows LLM to:**

1. Choose which tool to execute
2. Provide parameters using `_` placeholders  
3. Bind tool results with `as` keyword
4. Transform results in handler expression

:::
::::::::::::::

---

# Select Statement: Syntax

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let result = select {
    add(ctx, _, _) as sum => sum,
    subtract(ctx, _, _) as diff => diff
}
```

**Syntax pattern:**
```
function_call(params) as variable => expression
```

:::
::: {.column width="50%"}

**Key elements:**

- **Function call:** Can use `_` for LLM-populated params
- **`as variable`:** Binds result to variable name
- **`=> expression`:** Handler that processes result
- **Multiple clauses:** Comma-separated options for LLM

:::
::::::::::::::

---

# Select Statement: Tool Choice

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let result = select {
    add(ctx, _, _) as sum => sum,
    subtract(ctx, _, _) as diff => diff
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
- Execute function and store result as 'sum'
- Execute function and store result as 'diff'
```

**LLM chooses:** Index 1 (subtract) with params `(ctx, 2, 5)`

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

subtract(ctx, _, _) as diff => diff
```

:::
::: {.column width="50%"}

**Execution flow:**

1. LLM selected: `subtract(ctx, 2, 5)`
2. Function executes: `2 - 5 = -3`
3. Result bound: `diff = -3`
4. Handler evaluates: `diff` returns `-3`

:::
::::::::::::::

---

# Select Statement: Result Transformation

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
let result = select {
    add(ctx, _, _) as sum => {
        "Addition result: "!
        sum!
        sum
    },
    subtract(ctx, _, _) as diff => {
        "Subtraction result: "!
        diff!
        diff
    }
}
```

:::
::: {.column width="50%"}

**Handler expressions can:**

- Execute multiple statements
- Inject results into context
- Transform values
- Return modified results

**Last expression is return value**

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
        add(ctx, _, _) as sum => sum,
        subtract(ctx, _, _) as diff => diff
    }
    
    return result
}
```

:::
::: {.column width="50%"}

**For "Calculate 2 - 5":**

1. **Context:** Calculator prompt + user request
2. **Tool Choice:** LLM selects subtract clause
3. **Parameter Population:** LLM provides 2, 5
4. **Execution:** `subtract(ctx, 2, 5)` returns `-3`
5. **Binding:** `diff = -3`
6. **Handler:** Returns `diff` (-3)

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
        add(_, _) as sum => sum,
        subtract(_, _) as diff => diff
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

# If Statement Overview

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn has_data(ctx: Context) -> Boolean {
    "Is there data available to process?"!
}

fn check_value(ctx: Context) -> () {
    "Checking if we should process"!
    
    if has_data(ctx) {
        "Processing value"!
    }
    
    "Check complete"!
}
```

:::
::: {.column width="50%"}

**If statement features:**

- Conditional execution based on boolean
- Scoped block execution
- Variables declared in block are local
- Context flows into block

:::
::::::::::::::

---

# If Statement: Basic Usage

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn is_ready(ctx: Context) -> Boolean {
    "Is the system ready?"!
}

fn main(ctx: Context) -> () {
    "System initialization starting"!
    
    if is_ready(ctx) {
        "System is ready, proceeding"!
    }
    
    "After if statement"!
}
```

:::
::: {.column width="50%"}

**Execution flow:**

1. Call `is_ready(ctx)` → LLM returns boolean
2. If true, enter if block
3. Execute: `"System is ready, proceeding"!`
4. Exit if block
5. Execute: `"After if statement"!`

:::
::::::::::::::

---

# If Statement: Variable Scoping

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn needs_update(ctx: Context) -> Boolean {
    "Does the status need updating?"!
}

fn main(ctx: Context) -> () {
    let status = "initial"
    
    if needs_update(ctx) {
        "In if block"!
        status = "modified"
        let local_var = "block only"
    }
    
    status!
}
```

:::
::: {.column width="50%"}

**Scoping rules:**

- **Variable assignment:** Updates parent scope
- **New declarations:** Local to if block
- `status = "modified"` updates parent's `status`
- `local_var` only exists in if block
- Final `status` value is `"modified"`

:::
::::::::::::::

---

# If Statement: Placeholders in Condition

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn should_process(ctx: Context, data: String) -> Boolean {
    "Determine if we should process: "!
    data!
}

fn process_data(ctx: Context, data: String) -> () {
    "Processing: "!
    data!
}

fn main(ctx: Context) -> () {
    "You are a data processor"!
    "The input data needs validation"!
    
    if should_process(ctx, _) {
        "Processing data"!
        process_data(ctx, _)
    }
}
```

:::
::: {.column width="50%"}

**LLM populates placeholders:**

1. Context: "data processor" + "needs validation"
2. LLM fills `_` in `should_process(ctx, _)`
3. Function called with LLM-provided data
4. LLM generates boolean response
5. If true, enter block
6. LLM fills `_` in `process_data(ctx, _)`

**LLM provides data parameter and decides whether to process**

:::
::::::::::::::

---

# While Statement Overview

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(ctx: Context) -> () {
    let should_continue = true
    
    while should_continue {
        "Processing iteration"!
        
        should_continue = false
    }
    
    "Loop complete"!
}
```

:::
::: {.column width="50%"}

**While statement features:**

- Loop while condition is true
- Re-evaluate condition each iteration
- Scoped block like if statement
- Variable assignments affect parent scope

:::
::::::::::::::

---

# While Statement: Loop Control

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn main(ctx: Context) -> () {
    let counter = true
    
    while counter {
        "Loop iteration"!
        
        counter = false
    }
    
    "Loop exited"!
}
```

:::
::: {.column width="50%"}

**Execution flow:**

1. **Iteration 1:** `counter = true`, enter block
2. Execute: `"Loop iteration"!`
3. Set: `counter = false`
4. **Check condition:** `counter = false`, exit loop
5. Execute: `"Loop exited"!`

:::
::::::::::::::

---

# While Statement: Placeholders in Condition

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn task_complete(ctx: Context, progress: String) -> Boolean {
    "Is the task complete given progress: "!
    progress!
    "?"!
}

fn do_work(ctx: Context, step: String) -> String {
    "Performing step: "!
    step!
}

fn main(ctx: Context) -> () {
    "You are solving a complex task"!
    "Work through the problem step by step"!
    
    let result = ""
    
    while !task_complete(ctx, result) {
        "Working on next step"!
        result = do_work(ctx, _)
        result!
    }
    
    "Task finished"!
}
```

:::
::: {.column width="50%"}

**LLM controls loop with placeholders:**

1. Context: "solving a complex task"
2. `result` stores latest work result
3. Each iteration: `task_complete(ctx, result)`
4. LLM sees latest result, decides if done
5. Inside loop: `do_work(ctx, _)` 
6. LLM fills `_` with next step to perform
7. Result stored in `result`

**LLM sees latest result and controls when to stop**

:::
::::::::::::::

---

# Control Flow: Combined Example

:::::::::::::: {.columns}
::: {.column width="50%"}

```rust
fn should_start(ctx: Context) -> Boolean {
    "Should we start processing?"!
}

fn keep_going(ctx: Context, status: String) -> Boolean {
    "Should we continue given status: "!
    status!
    "?"!
}

fn process_step(ctx: Context) -> String {
    "Perform the next processing step"!
}

fn main(ctx: Context) -> () {
    "You are a task executor"!
    
    if should_start(ctx) {
        "Starting task"!
        let status = "started"
        
        while keep_going(ctx, status) {
            "Processing step"!
            status = process_step(ctx)
            status!
        }
        
        "Task complete"!
    }
}
```

:::
::: {.column width="50%"}

**LLM decision points:**

1. **If condition:** Should we start at all?
2. **While condition:** Continue given current status?

**LLM-controlled flow:**

- `should_start(ctx)` - LLM decides to begin
- `process_step(ctx)` - LLM performs work, updates status
- `keep_going(ctx, status)` - LLM sees status, decides to continue
- Loop exits when LLM returns false

**Enables adaptive, context-aware processes with state**

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
