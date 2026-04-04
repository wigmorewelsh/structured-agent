# Module System Phase 5

## Original Goal

Implement Phase 5 of the module system: module parameters with vtable dispatch. A module may declare typed parameters (`mod reporter(fmt: formatter::Formatter)`), call through them (`fmt::format(value)`), and have those calls resolved at runtime via a compile-time vtable. Type checking validates calls against the parameter's declared sig without knowledge of the concrete wiring. Diagnostics always refer to original source names.

## Status: Incomplete

The spec was not read before implementation. The vtable machinery, `::` separator, param sig resolution, and runtime dispatch are all correct. The wiring site syntax — the mechanism by which a caller binds a concrete module to a parameter slot — was not implemented. Without it, the concrete module is always inferred from the param path and cannot be overridden. Testing by substitution is impossible. See Phase 5a in `module-system-implementation.md`.

This phase also introduced `::` as the module path separator throughout the language, replacing `.` to unambiguously distinguish module paths from struct field access.

## What Was Done

### `::` Separator — Language-Wide Change

The original design used `.` for both module paths (`use greetlib.greet`) and struct field access (`point.x`). This created a parse ambiguity: `io.read()` could not be distinguished from field access without type information. Rather than add a specialised call instruction or thread type information into the parser, the separator for module paths was changed to `::` throughout.

**Parser (`compiler/parser.rs`)**

`parse_use` now uses `sep_by1(identifier_raw(), attempt(string("::")))`. `parse_module_param` does the same for the path in `mod name(param: path::Sig)`. `parse_call` also uses `sep_by1` with `::`, so `io::read(args)` produces `Call { function: "io::read" }` directly — unambiguous at the parse level, no type information required.

**AST display (`ast/mod.rs`)**

`Display` for `Definition::Use` and `Definition::ModuleHeader` uses `path.join("::")`.

**All qualified name strings**

Every place where a qualified function name is constructed or compared — `sigs.rs`, `wiring.rs`, `typecheck/checker.rs`, `compiler/mod.rs` — uses `::` as the separator. `check_visibility` tests for `contains("::")` rather than `contains('.')` to identify cross-module calls.

### `SigTable::sig_definitions`

`SigTable` gains a third field:

```rust
pub(crate) struct SigTable {
    pub(crate) visibility: ModuleVisibility,
    pub(crate) external_sigs: HashMap<String, FunctionSignatureTuple>,
    pub(crate) sig_definitions: HashMap<String, Vec<SigFunction>>,
}
```

`collect_sigs` in `compiler/sigs.rs` now folds over `Definition::Signature` nodes as well as function definitions, populating `sig_definitions` in the same pass.

### Type Checker Param Sig Resolution (`typecheck/checker.rs`)

`check_module_with_external_sigs` accepts a new `sig_definitions` argument. Before `collect_function_signatures` runs, `register_param_sigs` is called. It inspects any `ModuleHeader` on the module, and for each param:

- If the sig name appears in `sig_definitions`, the sig's functions are used.
- Otherwise, the concrete module's public exports are derived from `external_sigs` by stripping the `concrete_module::` prefix.

In either case the functions are registered in `function_signatures` under `param::fn` keys (e.g. `io::read`). The type checker then checks `io::read()` call sites against those signatures without knowing the concrete wiring. No changes to call-checking logic were needed.

### Vtable Construction (`compiler/wiring.rs`)

`compiler/wiring.rs` is a new file. It exports:

```rust
pub(crate) type Vtables = HashMap<String, HashMap<String, String>>;

pub(crate) fn resolve_vtables(modules: &[ParsedModule], sig_table: &SigTable) -> Vtables
```

`resolve_vtables` folds over all parsed modules. For each module that has a `ModuleHeader` with params, it builds a vtable: a map from `param::fn` → `concrete::fn` for every function in the param's sig (named or derived from exports). The result is keyed by module name.

### `CompiledProgram::vtables` (`compiler/mod.rs`)

`CompiledProgram` gains `vtables: Vtables`. A `with_vtables` builder method and a `vtables()` accessor are added. `resolve_vtables` is called after all modules are type-checked and emitted, and the result is stored via `with_vtables`.

### `CompiledFunction::module_name` (`bytecode/compiler.rs`)

`CompiledFunction` gains `module_name: Option<String>`, defaulting to `None`. `emit_module` sets it to the module prefix when emitting non-entry module functions — this is the mechanism by which the VM knows which module a function belongs to at dispatch time.

### Vtable Dispatch (`bytecode/vm.rs`)

`execute_call` receives `module_name: Option<&str>` from the currently-executing `CompiledFunction`. Before the registry lookup, it checks:

```rust
let resolved_name = module_name
    .and_then(|m| self.runtime.vtables().get(m))
    .and_then(|vtable| vtable.get(function_name))
    .map(String::as_str)
    .unwrap_or(function_name);
```

The bytecode instruction `Call { function_name: "io::read" }` is emitted unchanged. The substitution happens entirely at dispatch, preserving original source names in all diagnostics and bytecode.

### `Runtime::vtables` (`runtime/engine.rs`)

`Runtime` gains a `vtables: HashMap<String, HashMap<String, String>>` field, initially empty. In `run()`, after `compile()` returns, `runtime.vtables = compiled_program.vtables().clone()` is set on the local runtime ref before function registration. The VM receives the runtime as `Arc<Runtime>` and calls `self.runtime.vtables()` at dispatch.

Function registration in `run()` was also corrected: previously it used `register_function`, which re-keyed by the function's own internal name. It now iterates `compiled_program.functions()` and inserts directly into `function_registry` by map key. This was necessary for `use` aliases to work at runtime.

### `use` Alias Runtime Registration (`compiler/mod.rs`)

A pre-existing gap was exposed by the first end-to-end multi-module execution test: `use greetlib::greet` aliased `greet` in the type checker but the runtime only had `greetlib::greet` registered. The fix:

`emit_module` collects `use_aliases: Vec<(String, String)>` in `ModuleArtifact` for entry-module `use` declarations. `CompiledProgram` accumulates them as `pending_aliases` during `merge`, then `apply_pending_aliases()` is called after all modules are merged. It walks the aliases and clones the function arc under the alias key. `Runtime::run()` then registers by map key (not by the function's own name), so the alias reaches the registry correctly.

### Samples (`samples/modules/`)

Three sample files demonstrate the module system with the print engine:

- `greeter.sa` + `main.sa` — basic multi-module: `use greeter::greet`, `use greeter::farewell`, called from `main`.
- `formatter.sa` + `reporter.sa` + `wired.sa` — vtable dispatch: `reporter` declares `mod reporter(fmt: formatter::Formatter)`, calls `fmt::format(value)`, wired via `wired.sa` which imports `reporter::report`. At runtime the vtable resolves `fmt::format` → `formatter::format`.

Run with:

```
cargo run -p structured-agent -- run --file src/structured-agent/samples/modules/main.sa
cargo run -p structured-agent -- run --file src/structured-agent/samples/modules/wired.sa
```

## What Is Not Done

### Wiring site syntax — the spec was not read

The spec defines an explicit binding site syntax. The caller binds a name to a concrete module at the declaration site and passes that binding into the parameterised module:

```
mod io: storage::Storage = storage::disk
mod db(io)
```

This was not implemented. Instead, the concrete module is silently inferred from the first element of the param path (`formatter` from `formatter::Formatter`). This means:

- The dependency is fixed at the point the parameterised module is written, not at the point it is used.
- There is no way to pass a different concrete module — a test double, an alternative implementation — at a wiring site.
- The primary value of the phase (testing by substitution) is unreachable.

The `wired.sa` sample appears to work but does not exercise the spec'd mechanism. It works because `formatter` happens to be the module named in the path and also the only available implementation. It would silently break if a different module were intended.

What needs implementing is documented in Phase 5a of `module-system-implementation.md`. The short list: parser support for `mod name: Sig = impl` and `mod db(bound_name)`, AST representation of binding declarations, orchestrator collection of bindings before vtable construction, and satisfaction checking at the binding site.

### Phase 6 — Module Contract Matching (blocked on Phase 5a)

`mod mock: storage::disk` structural contract verification is not started. See `module-system-implementation.md`.

### `pub use` re-export semantics

`pub use` is parsed and `is_pub` is set on the AST node, but the type checker makes no distinction between `use` and `pub use` at cross-module call sites.

### Sig satisfaction checking

A module declaring `mod name: SomeSig` is not verified to actually implement all functions in the sig. The type checker does not yet run a satisfaction pass against the module's own exports.

## Layer Diagram

```
cli/app.rs
    |
    | ProgramSource
    v
runtime/engine.rs  (Runtime::builder, run, check)
    |
    | compile_source / compile_file
    v
compiler/mod.rs  (Compiler::compile)
    |
    +-- discovery.rs      discover() -> Vec<ParsedModule>
    +-- sigs.rs           collect_sigs() -> SigTable          (sig_definitions added)
    +-- type_check_module()  -> Result<(), TypeError>         (register_param_sigs)
    +-- analyse_module()     -> Vec<Warning>
    +-- emit_module()        -> ModuleArtifact                (module_name on CompiledFunction)
    +-- wiring.rs         resolve_vtables() -> Vtables        (new)
    +-- CompiledProgram::merge() + apply_pending_aliases()
    |
    v
runtime/engine.rs  (vtables stored on Runtime)
    |
    v
bytecode/vm.rs  (execute_call vtable resolution)
```

## Known Cleanup Opportunities

The `pending_aliases` / `use_aliases` / `apply_pending_aliases` machinery in `CompiledProgram` is a workaround for the ordering dependency between module merges and alias resolution. A cleaner approach would be to make alias resolution a named phase function like the others, taking `&[ParsedModule]` and producing a supplementary name map the orchestrator applies directly.

`FunctionSignatureTuple` remains a three-tuple `(Vec<Parameter>, AstType, bool)`. The `bool` is `is_pub`. This would be cleaner as a named struct.

## See Also

- [module-system-implementation.md](../module-system-implementation.md)
- [0001-module-system-phases-1-4.md](0001-module-system-phases-1-4.md)
- `src/structured-agent/src/compiler/wiring.rs`
- `src/structured-agent/src/compiler/sigs.rs`
- `src/structured-agent/src/typecheck/checker.rs`
- `src/structured-agent/src/bytecode/vm.rs`
- `src/structured-agent/src/runtime/engine.rs`
- `src/structured-agent/samples/modules/`
