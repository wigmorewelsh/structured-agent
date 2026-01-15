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
