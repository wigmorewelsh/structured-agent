# IL Analysis Framework

## Original Goal

Add a post-compilation analysis pass that inspects bytecode after the compiler emits it, following the same pattern as the existing AST-level analysers in `src/analysis/`. The requirement was two initial analysers — one checking every variable is allocated before use, one checking every variable is dropped after use — with each analyser in a separate file and the whole system wired into the compiler pipeline.

The secondary goal, which shaped the architecture, was to avoid boxing bytecode functions unnecessarily. The discussion that preceded the work noted that `CompiledProgram` stored its functions as `Box<dyn ExecutableFunction>` not because they needed to be polymorphic at that stage, but because native and user-defined functions had been conflated in the same collection. Separating them at the compiler level was a prerequisite for clean IL analysis.

## What Was Done

### Compiler Type Refactoring

`ModuleArtifact` and `CompiledProgram` previously stored functions as `Vec<Box<dyn ExecutableFunction>>` and `HashMap<String, Box<dyn ExecutableFunction>>` respectively. The compiler only ever places `BytecodeFunctionExpr`-wrapped `CompiledFunction` values there; native functions are registered separately at runtime via `NativeFunctionProvider`. The fields were changed to hold `CompiledFunction` directly.

```rust
// before
pub struct CompiledProgram {
    functions: HashMap<String, Box<dyn ExecutableFunction>>,
    ...
}

// after
pub struct CompiledProgram {
    functions: HashMap<String, CompiledFunction>,
    ...
}
```

`CompiledFunction` gained a `Debug` derive to satisfy the `#[derive(Debug)]` on `CompiledProgram`. The `merge` method now uses `f.name.clone()` instead of calling through the `Function` trait, and `apply_pending_aliases` uses `.cloned()` instead of `.clone_executable()`. The `ExecutableFunction` and `Function` imports were removed from `compiler/mod.rs`.

Wrapping into `BytecodeFunctionExpr` was pushed to the two consumer sites: `runtime/engine.rs` when registering compiled functions with the runtime, and `bytecode/tests.rs` in the one test that constructs a runtime from a `CompiledProgram`. The `main.rs` binary, which re-declares all modules directly rather than using the library crate, required `mod il_analysis;` to be added alongside the existing declarations.

### The `il_analysis` Module

The module lives at `src/il_analysis/` and is declared as `pub mod il_analysis` in both `lib.rs` and `main.rs`. It mirrors the structure of `src/analysis/`: one file per analyser, a matching `_test.rs` file per analyser, and a `mod.rs` that owns the shared trait, the warning enum, the shared instruction helpers, and the runner.

The central trait is:

```rust
pub trait IlAnalyzer {
    fn name(&self) -> &str;
    fn analyze_function(&mut self, function: &CompiledFunction) -> Vec<IlWarning>;
}
```

Two module-level helpers enumerate the variable reads and writes for any `Instruction` variant, shared across all analysers:

```rust
pub(crate) fn instruction_reads(instruction: &Instruction) -> Vec<&str>
pub(crate) fn instruction_writes(instruction: &Instruction) -> Option<&str>
```

`IlWarning` carries a `message()` method and a `to_diagnostic()` method returning `Diagnostic<FileId>` with no source labels, which is sufficient for the reporter to emit it to stderr.

`IlAnalysisRunner` accepts analysers via a builder pattern identical to `AnalysisRunner` in the AST layer.

### Analysers

Seven analysers were implemented, each inspired by checks found in established IL verification tools such as .NET ILVerify, WebAssembly validation, and LLVM's IR analysis passes.

**`VariableAllocationAnalyzer`** seeds an allocated set from the function's declared parameters, then walks instructions linearly. For each instruction it checks every read operand against the allocated set before recording any write. It catches variables used as a source before a `Decl` appears for them.

**`VariableDropAnalyzer`** collects the sets of `Decl`'d, `Drop`'d, and `Ret`'d variables in a single pass and warns for any variable in the declared set that appears in neither the dropped nor the returned set.

**`BranchTargetAnalyzer`** (inspired by WebAssembly validation and .NET ILVerify) checks that every offset in `Br`, `BrFalse`, `BrTrue`, and `Switch` instructions falls within `[0, instructions.len())`. The VM uses these offsets as direct absolute indices with `offset as usize`; an out-of-range value causes a runtime panic with "PC out of bounds".

**`ReturnCoverageAnalyzer`** (inspired by LLVM's terminator rule) warns when a non-empty function contains no `Ret` instruction. Without one the VM's main loop terminates with the same "PC out of bounds" error.

**`ContextBalanceAnalyzer`** (inspired by .NET stack balance and WebAssembly's structured block model) tracks a depth counter incremented by `CtxChild` and decremented by `CtxRestore`. A decrement below zero is reported as `ContextUnderflow`. A positive counter at function exit is reported as `ContextNotRestored` with the remaining depth.

**`UnreachableInstructionAnalyzer`** (inspired by LLVM dead code elimination) first collects all instruction indices that are the target of any branch, then walks the instruction list with a reachability flag. Only `Ret` and unconditional `Br` clear the flag; conditional branches (`BrFalse`, `BrTrue`, `Switch`) do not, because their fall-through path remains live. An instruction that is a branch target restores the flag regardless of what precedes it.

**`DuplicateDeclAnalyzer`** (inspired by LLVM's SSA single-definition rule) warns when the same variable name appears in more than one `Decl` instruction.

**`DoubleDropAnalyzer`** (inspired by Rust's borrow checker) warns when the same variable name appears in more than one `Drop` instruction.

**`CallArityAnalyzer`** (inspired by LLVM call-site type checking and .NET ILVerify) is the only analyser that requires cross-function information. It is constructed with a `HashMap<String, usize>` mapping each known function name to its parameter count. Unknown names (external functions, native functions, MCP tools) are silently skipped. It is built in `analyse_il` from the same `functions` map that is being analysed:

```rust
let arities = functions
    .iter()
    .map(|(name, f)| (name.clone(), f.parameters.len()))
    .collect();
```

### Pipeline Integration

`analyse_il` runs after `compiled.apply_pending_aliases()` in the `Compiler::compile` method. It builds a runner containing all nine analysers and iterates over every compiled function. Warnings are emitted through the existing `DiagnosticReporter` using the same call pattern as AST-level warnings.

```rust
let il_reporter = diagnostics.reporter().clone();
for warning in analyse_il(compiled.functions()) {
    if let Err(io_err) = il_reporter.emit_diagnostic(&warning.to_diagnostic()) {
        eprintln!("Failed to emit IL warning: {}", io_err);
    }
}
```

## Findings Against the Sample Programs

Running `check` against every `.sa` file in `samples/` with the new analysers active produced two categories of finding beyond the pre-existing variable-drop noise.

### Unreachable Instructions (real dead code in the compiler)

Several samples produced warnings of the form `instruction N is unreachable`. The cause is `compile_if_statement` in `src/bytecode/compiler.rs`. When the body of an `if` block contains a `return` statement, `compile_return_statement` emits a `Ret` instruction. The outer `compile_if_statement` then unconditionally emits `CtxRestore` and `Br end_label` after the body loop, regardless of whether the body already terminated. The result is:

```
Ret $tmp           <- from the return statement
CtxRestore         <- unreachable
Br end_if          <- unreachable
```

Neither the `CtxRestore` nor the `Br` will ever execute on that path. The same pattern appears in the `else` branch. Affected samples include `agent.sa`, `agent-recursive.sa`, and `structs/field_as_condition.sa`. This is a genuine compiler deficiency: context cleanup after an early return is skipped, which in a future runtime that inspects context depth would cause incorrect behaviour.

The relevant code is `compile_if_statement` at `src/bytecode/compiler.rs` lines 164–207. The fix is to inspect whether the last emitted instruction within a body is already a terminator before emitting the trailing `CtxRestore` and `Br`.

### Duplicate Declarations (false positives from the linear analyser)

Warnings of the form `variable r declared a second time at instruction N` appear for every `select` expression and for `let` bindings inside `while` loop bodies. Both are false positives caused by the analyser's lack of control-flow graph awareness.

In `compile_select_expression` (lines 418–508 of `compiler.rs`), the result variable for each clause (e.g. `r` from `edit_file(_) as r => r`) receives a `Decl` inside each branch's `CtxChild/CtxRestore` block. Because the `Switch` instruction routes execution to exactly one branch, only one `Decl` for `r` ever executes at runtime. The linear analyser sees all `Decl` instructions in sequence and reports a duplicate for each branch after the first.

In `compile_while_statement` (lines 209–238), the condition variable and any `let` bindings inside the loop body receive a `Decl` on every pass through the loop. The VM's `declare_variable` simply overwrites the slot, so runtime behaviour is correct. The analyser reports a duplicate on the second and subsequent iterations' `Decl` instructions.

A control-flow aware implementation would compute the dominator tree and report a duplicate `Decl` only when one definition dominates another on the same path, which is the standard SSA-validity check.

### Variable-Drop Warnings (pre-existing, widespread)

The flood of `variable X is allocated but never dropped` warnings predates this work. They were visible in the earlier integration run before the new analysers were added. Named variables from `let` bindings, select result variables (`r`, `result`), and many `$tmpN` temporaries from `Call` parameters and `StructGet` chains are never `Drop`'d. This is not a correctness problem at runtime — the VM does not enforce explicit cleanup — but it represents incomplete codegen that would matter if the runtime ever implemented reference counting or scope-bounded resource management.

## Known Limitations

All analysers except `CallArityAnalyzer` perform a single linear pass with no control-flow graph. This is sufficient to catch bugs where the compiler emits structurally invalid sequences, but it produces false positives wherever the compiler uses branching to create mutually exclusive paths that happen to share a variable name. The `DuplicateDeclAnalyzer` findings in select and while bodies are the clearest examples of this.

`CallArityAnalyzer` only checks calls to functions compiled in the same program. Calls to external functions and native functions are silently ignored because their arities are not available in the `CompiledFunction` map.

`ContextBalanceAnalyzer` counts `CtxChild` and `CtxRestore` across the entire function linearly. In a function with branching paths that push different context depths, this count may be misleading. A path that enters a `CtxChild` and then takes an early `Ret` would not have its `CtxRestore` executed; the linear counter would report `ContextNotRestored` even though the compiler may intend cleanup to happen elsewhere. No such warning was observed in the current samples, but the analyser could produce false positives in more complex control flow.

## Files Changed or Created

| Path | Change |
|---|---|
| `src/il_analysis/mod.rs` | Created. Trait, warning enum, shared helpers, runner. |
| `src/il_analysis/variable_allocation.rs` | Created. |
| `src/il_analysis/variable_allocation_test.rs` | Created. |
| `src/il_analysis/variable_drop.rs` | Created. |
| `src/il_analysis/variable_drop_test.rs` | Created. |
| `src/il_analysis/branch_target.rs` | Created. |
| `src/il_analysis/branch_target_test.rs` | Created. |
| `src/il_analysis/return_coverage.rs` | Created. |
| `src/il_analysis/return_coverage_test.rs` | Created. |
| `src/il_analysis/context_balance.rs` | Created. |
| `src/il_analysis/context_balance_test.rs` | Created. |
| `src/il_analysis/unreachable_instructions.rs` | Created. |
| `src/il_analysis/unreachable_instructions_test.rs` | Created. |
| `src/il_analysis/duplicate_decl.rs` | Created. |
| `src/il_analysis/duplicate_decl_test.rs` | Created. |
| `src/il_analysis/double_drop.rs` | Created. |
| `src/il_analysis/double_drop_test.rs` | Created. |
| `src/il_analysis/call_arity.rs` | Created. |
| `src/il_analysis/call_arity_test.rs` | Created. |
| `src/lib.rs` | Added `pub mod il_analysis`. |
| `src/main.rs` | Added `mod il_analysis`. |
| `src/bytecode/compiler.rs` | Added `#[derive(Debug)]` to `CompiledFunction`. |
| `src/compiler/mod.rs` | Replaced `Box<dyn ExecutableFunction>` fields with `CompiledFunction`. Removed `ExecutableFunction` and `Function` imports. Added `analyse_il` function and IL analyser imports. Wired the IL pass into `Compiler::compile`. |
| `src/runtime/engine.rs` | Imports `BytecodeFunctionExpr`. Wraps `CompiledFunction` at the point of registration and when running main. |
| `src/bytecode/tests.rs` | Updated `vm_execution_tests` to import `BytecodeFunctionExpr` and wrap when registering with the runtime. |

## Possible Improvements

The most impactful improvement would be a basic control-flow graph over the instruction list. The `Switch` offsets, `Br` offsets, and fall-through edges are all statically available; constructing a CFG from them is straightforward. With a CFG, `DuplicateDeclAnalyzer` could check per-path dominance, eliminating the select and while false positives, and `ContextBalanceAnalyzer` could verify balance on each path independently rather than across the whole function.

The unreachable-after-return defect in `compile_if_statement` and `compile_while_statement` warrants a fix in the compiler rather than a suppression in the analyser. The simplest fix is a `terminates` predicate on a compiled body that returns true when the last statement is a `return`, suppressing the trailing `CtxRestore` and `Br` emission.

The `VariableDropAnalyzer` findings are numerous enough that they obscure other output. The codegen should be audited to ensure temporaries from `Call` parameters and chained field accesses are dropped after their last read. The `compile_call_expression` path in particular does not drop its argument temporaries after the call, unlike `compile_injection` which correctly drops its temporary after `CtxEvent`.

The `IlWarning::to_diagnostic` method produces warnings with no source labels because the IL has no span information. Attaching a function name to each warning — either as a field on `IlWarning` or by having the runner annotate warnings after collection — would make the output more actionable when a project contains many functions.