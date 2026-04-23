# Module Parameter Implementation

This document covers the implementation of module parameters in the SA compiler. It is informed by the tracer bullet spike in `docs/handover/0003-module-parameter-tracer-bullet-complete.md`, which proved the runtime model works and identified the problems with the old syntax that led to the design described here.

## Syntax

A module declares its dependencies in its header:

```/dev/null/db.sa#L1-4
mod db(io: storage.Storage)

use io::read
use io::query as io_query

pub fn connect(): Connection { read("config") }
pub fn query(q: String): List<Row> { io_query(q) }
```

A caller passes implementations at the `use` site, positionally or by name:

```/dev/null/app.sa#L1-2
use db(storage.disk)::connect
use db(io: storage.disk)::query
```

Named and positional args can be mixed. Named args match against the header param list by name; positional args match by index. All declared params must be supplied — no partial application.

When the same parameterised module is needed with two different implementations in the same file, `as` aliases distinguish the imported names:

```/dev/null/app.sa#L1-3
use db(storage.disk)::query
use db(storage.memory)::query as query_mem
```

## Runtime Model

The spike confirmed that the runtime mechanism is correct and should be retained unchanged. The bytecode instruction set has two relevant instructions:

- `LoadModule { name }` — loads a module reference into a local variable
- `CallModuleBytecode { module_var, function_name }` — dispatches a call through a module reference

These live in `src/structured-agent/src/bytecode/instruction.rs`. VM dispatch is in `src/structured-agent/src/bytecode/vm.rs`.

When a function imports `use io::read` and `io` is a module parameter, the compiler emits `LoadModule` for the wired concrete module, stores the reference in a local, and emits `CallModuleBytecode` for each call to `read`. The function body is unaware of which concrete module is loaded — substitution happens at the call site, not inside the callee. This plumbing also serves as the foundation for trait witness dictionaries later.

One constraint that must be preserved: `CallModuleBytecode` can only dispatch to bytecode functions. The VM's `execute_module_call` constructs a `FunctionName` from the loaded module reference and looks up functions via `resolved_name.to_string()`, which produces `"module::function"`. Native functions are registered under their short name (`"function"` not `"module::function"`), so the lookup fails for externals. The elaboration phase must not emit `ModuleBound` dispatch for external functions. Until the VM lookup strategy is unified across `execute_module_call` and `execute_external_call`, this constraint stands.

## Spike Issues and Their Resolution

The spike introduced several workarounds to make the old syntax work. This section maps each one to its outcome under the new design.

**Empty-string sentinel in `ModuleBinding`.** The parser produced `NonEmpty::new(String::new())` when a bare contract name had no path prefix, creating a hidden invariant detectable only via `sig_path.first().as_str() == ""`. Resolved: `Definition::ModuleBinding` is removed entirely. No sentinel, no hidden invariant.

**`ModuleBinding` splits one concept across two fields.** `sig_path: NonEmpty<String>` and `sig_name: String` were stored separately, requiring mental reconstruction of the full contract path. Resolved: `ModuleBinding` is removed. `UseParam` carries the full implementation path as a single `NonEmpty<String>`.

**`find_wiring_substitution` performs a full scan on every call.** The helper iterated over all modules in the symbol table to find which wiring site targeted the module being compiled. Resolved: under the new syntax the param binding is local to the `use` statement, so no cross-module scan is needed. `find_wiring_substitution` is deleted.

**`find_wiring_substitution` assumes at most one wiring site.** It returned the first match from hash map iteration, silently producing wrong results when two composition roots wired the same module with different implementations. Resolved: each `use` statement is its own instantiation. Two `use` statements with different params are two independent bindings, disambiguated by `as` aliases at the item level. The first-match-wins bug cannot arise.

**`resolve_module_path` had two concerns.** Before the spike it resolved a path segment in a `use` statement to a concrete module name. The spike added wiring substitution logic to the same function, mixing two operations. Resolved: substitution is no longer needed as a separate pass. `resolve_module_path` resolves the param binding from the `use` statement itself, which is the same operation as resolving any other path segment.

**`resolve_function_alias` called twice per call expression.** In `elaborate_call`, `resolve_function_alias` was called explicitly to determine dispatch kind, then `resolve_function_call` was called for the resolved name and signature — but `resolve_function_call` calls `resolve_function_alias` internally as its first step. The second call hit the salsa cache so it was harmless, but confusing. In the new implementation, derive dispatch from the result already returned by `resolve_function_call`. Do not call `resolve_function_alias` separately in `elaborate_call`.

**Typed AST conflates `kind` and `dispatch`.** `typed_ast::Expression::Call` carries both `kind: FunctionKind` and `dispatch: CallDispatch`. The bytecode compiler uses `dispatch` to choose between direct and module-bound paths, and `kind` only inside the `Direct` branch to choose between `CallBytecode` and `CallExternal`. The `ModuleBound` variant only ever appears with bytecode functions but the encoding does not enforce this. Consider replacing the two fields with a single three-variant enum — `DirectBytecode`, `DirectExternal`, `ModuleBound` — that makes the constraint explicit in the type. This is not a blocker but should be done before the typed AST grows further consumers.

**Contract checking was structurally incomplete.** For bare contracts the spike only verified that the implementation module exists in the symbol table, not that it exports all the functions the contract requires. A structural mismatch would only surface at runtime. The full implementation must perform a structural check: for a named sig contract, verify the supplied module exports all functions declared in the sig; for a concrete module contract, verify the supplied module exports all public functions of the referenced module.

**Discovery missed `ModuleBinding`.** The discovery pass scanned `Use` and `ModuleHeader` definitions but not `ModuleBinding`, so implementation modules were never queued for loading until the spike added the arm. Resolved by removal of `ModuleBinding`. The lesson: every AST node that references a module by path must have a corresponding arm in `referenced_module_names`. When `Use` gains a `params` field, that field must be included in `referenced_module_names` — emit one `ImportType::Absolute` per param path.

**`ModuleBound` dispatch gate was wrong.** The spike gated `ModuleBound` on `FunctionKind::Bytecode`. This was a symptom fix. The correct gate is whether the resolved function came from a `use` statement that carries params. A function imported via `use db(storage.disk)::connect` is module-bound; one imported via `use io::print` is not. Implement this gate directly rather than re-deriving it from `FunctionKind`.

## Compiler Phases

### AST

`Definition::ModuleBinding` and `Definition::WiringSite` are removed from `src/structured-agent/src/ast/mod.rs`.

`Definition::Use` gains a `params` field:

```
params: Vec<UseParam>
```

`UseParam` is an enum with two variants:

```
Positional(NonEmpty<String>)
Named { name: String, path: NonEmpty<String> }
```

An empty `Vec` means no params, preserving existing behaviour for all unparameterised imports. All `match` sites on `Definition` that currently have `ModuleBinding` and `WiringSite` arms need updating — most become exhaustiveness fixes only, since neither variant produced output in those sites.

### Parser

`parse_use` in `src/structured-agent/src/compiler/parser.rs` is extended to handle an optional param list between the module name and `::`:

```
use <module> ( <param_arg> (, <param_arg>)* )? :: <name> (as <alias>)?
```

Where `<param_arg>` is `path` (positional) or `name: path` (named). Paths use `::` as separator, consistent with the existing `use` path syntax.

`parse_module_binding` and `parse_wiring_site` are deleted. `parse_program` attempts these parsers by position in the file — those attempts are removed.

The current `parse_use` requires at least one `::` segment (`many1` on the rest). This restriction can stay; a bare `use module` without any `::name` is not valid syntax. The param list, if present, sits between the first segment and the first `::`.

### Discovery

In `referenced_module_names` in `src/structured-agent/src/compiler/discovery.rs`, the `Definition::Use` arm must emit one `ImportType::Absolute` per param path in addition to the existing relative import for the module path itself. The existing `ModuleBinding` arm is removed. The `ModuleHeader` arm that emits absolute imports for header params is unchanged — it continues to handle the case where the parameterised module itself needs to discover its contract modules.

### Type Checking

**`check_definition`** in `src/structured-agent/src/typecheck/synthesize.rs`. Remove the `ModuleBinding` and `WiringSite` arms. Extend the `Use` arm: when `params` is non-empty, resolve the target module's header from the symbol table, match each `UseParam` to a header param positionally or by name, and verify structural satisfaction for each. Produce a clear diagnostic if params are supplied for a module with no header params, or if params are omitted for a module that declares them.

**`resolve_module_path`** in `src/structured-agent/src/typecheck/db.rs`. When resolving a path that matches a module header parameter name, the concrete module to load is no longer found via `find_wiring_substitution`. Instead, the `use` statement that imported the function being resolved carries the param binding directly. The resolver needs access to the `UseParam` list from the relevant `use` statement to perform this substitution. How that context is threaded into `resolve_module_path` is the main design question for this phase: the param bindings must be available at the point of resolution without a global scan.

`find_wiring_substitution` is deleted.

**Error variants.** `UnknownModuleBinding` and `ModuleBindingTypeMismatch` in `src/structured-agent/src/typecheck/error.rs` are removed. Replace with `UnknownModuleParam` (supplied module does not exist) and `ModuleParamMismatch` (supplied module does not satisfy the declared sig).

### Elaboration

In `elaborate_call` in `src/structured-agent/src/typecheck/elaboration.rs`, derive `CallDispatch` from whether the `use` statement that resolved the function carries params — not from `FunctionKind`. Do not call `resolve_function_alias` separately; derive all information from the result returned by `resolve_function_call`.

## Test Coverage

The integration test `test_file_based_module_binding_tracer_bullet` in `src/structured-agent/tests/integration/integration_test.rs` exercises the full pipeline. The fixture files in `src/structured-agent/tests/integration/fixtures/module-binding-tracer/` use the old spike syntax and must be rewritten to the new `use(params)::item` form as the first step of the implementation, before any compiler changes, so the test acts as a failing target throughout.

Parser tests for `parse_use` should cover: bare use unchanged, positional single param, positional multiple params, named single param, named multiple params, mixed named and positional, and `as` aliasing with params. The existing `test_parse_module_binding` and `test_parse_wiring_site` tests are deleted.

Typecheck tests should cover: valid param satisfying a named sig, valid param satisfying a concrete module contract, param that does not satisfy the sig, params supplied for a module that declares none, params omitted for a module that requires them.

## See Also

- `docs/module-system.md` — language-level design including header forms and the sig/structure distinction
- `docs/handover/0003-module-parameter-tracer-bullet-complete.md` — full spike record including the five problems solved and all technical debt introduced
- `src/structured-agent/src/bytecode/instruction.rs` — `LoadModule` and `CallModuleBytecode` definitions
- `src/structured-agent/src/bytecode/vm.rs` — `execute_module_call` and `execute_external_call` dispatch