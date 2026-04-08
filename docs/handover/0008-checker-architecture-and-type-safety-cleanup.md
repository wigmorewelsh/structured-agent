# Checker Architecture and Type-Safety Cleanup

## Original Goal

The work in this session had two overlapping goals. The first was to implement the six-stage checker architecture roadmap described in `docs/checker-architecture-roadmap.md`, which aimed to make the metadata table the authoritative source for all symbol information and to split the monolithic `checker.rs` into focused modules. The second was a series of code quality improvements that emerged during review: replacing string-based representations of module imports with typed structs, removing bare-name type lookups from the symbol query interface, enforcing proper encapsulation across the typecheck module, and eliminating uses of deprecated API.

## What Was Done

### Stages 1–6 of the Checker Architecture Roadmap

All six stages were completed.

Stage 1 moved call-site vtable substitution from `compiler/wiring.rs` into `check_call` in the typechecker. The substitution now happens during elaboration rather than as a post-hoc mutation pass. `wiring.rs` was deleted entirely, along with `reconstruct_typed_modules`, `sync_lowered_to_metadata`, and the `lower_typed_module` loop in `compile()`.

Stage 2 replaced `emit_module` and `ModuleArtifact` with direct reads from `MetaData<BytecodeRefs>`. Use aliases were moved into `ModuleDefinition` (later replaced by the typed `use_imports: Vec<UseImport>` described below). `CompiledProgram` was reduced to `metadata`, `main_function`, `source_path`, and `module_visibility`.

Stage 3 added `compiled: Arc<OnceLock<Result<CachedProgram, String>>>` to `Runtime`. The program is now compiled once and cached. `run_with_handle` no longer rebuilds a `function_registry` on every call. `get_function` dispatches on demand from the cache.

Stage 4 populated `TypeDefinitionKind::Function` for every registered function, making `FunctionDefinition.type_name` a genuine foreign key into `metadata.types`. New tests assert that `metadata.type_def(fn_def.type_name)` returns the correct kind with matching parameters and return type.

Stage 5 removed the semantic marker variants `Builtin` and `ModuleParamBinding` from `CheckerAstRef`. A new `NoAst` struct (implementing `AstRef`) and `PrimitiveRefs` type were introduced. Primitive type entries and module parameter bindings are now held in separate collections on `TypeChecker` (`primitive_types` and `param_bindings`) rather than in the shared metadata.

Stage 6 split `checker.rs` (over 2 300 lines) into six files:

- `typecheck/mod.rs` — `TypeChecker`, `CheckContext`, `TypeEnvironment`, `FunctionSignature`, `check_modules`, `function_kinds`
- `typecheck/refs.rs` — all reference types
- `typecheck/collection.rs` — passes 1 and 2
- `typecheck/query.rs` — symbol lookups
- `typecheck/constraints.rs` — unification
- `typecheck/elaboration.rs` — type-directed elaboration

### Code Quality Work

Several rounds of review-driven improvement followed the roadmap work.

The `type_by_name` and `trait_by_name` methods were removed from the `SymbolQuery` trait. These methods performed a linear scan returning the first type or trait whose bare name matched, which would silently return the wrong result if two modules defined types with the same name.

The string-based representation of module imports (`use_aliases: Vec<(String, String)>`) was replaced with a typed `UseImport` struct in `symbols.rs`:

```rust
pub struct UseImport {
    pub local: String,
    pub module: ModuleName,
    pub name: String,
}
```

`ModuleDefinition.use_imports` now holds `Vec<UseImport>`. `AliasToQualified` in the typechecker was changed from `HashMap<String, String>` to `HashMap<String, UseImport>`. All callers that previously used `rsplit_once("::")` to recover the module and name from a concatenated string now access `import.module` and `import.name` directly.

`CheckContext` gained a `type_imports: &HashMap<String, UseImport>` field. The functions `get_struct_fields`, `get_trait_functions`, and `get_sig_functions` in `query.rs` were changed to resolve names through this map rather than through a bare-name scan. A call to `get_struct_fields("Foo", current_module, type_imports)` now looks up the import entry for `"Foo"` if one exists, falling back to the current module; no linear scan of the types table is performed. `type_by_name` and `trait_by_name` were subsequently deleted from `MetaData` entirely.

`Definition::Use` in the AST was restructured. The flat `path: Vec<String>` that conflated the source module path with the imported name was replaced with `path: NonEmpty<String>` (the module path) and `name: String` (the imported name). The parser was updated to enforce that a `use` statement must have at least one `::`, making a bare `use Foo` a parse error. The same split was applied to `Definition::ModuleBinding`, which now carries `sig_path: NonEmpty<String>`, `sig_name: String`, and `impl_path: NonEmpty<String>` in place of two flat `Vec<String>` fields. The `path.len() >= 2` guards and `path.last().unwrap()` calls that appeared in `collection.rs`, `query.rs`, `discovery.rs`, and `mod.rs` are gone.

`FunctionName::parse` was introduced in `symbols.rs` as the non-deprecated replacement for `from_qualified_str`. It uses `ModuleName::new` internally. `engine.rs` was updated to use `FunctionName::parse` and `ModuleName::new(nonempty!["main".to_string()])` in place of the deprecated calls.

Visibility across the typecheck module was audited. In `elaboration.rs`, twelve functions that are only called within the file had their `pub(super)` removed. In `collection.rs`, `runtime_type_to_ast` and `insert_fn` were made private. In `query.rs`, `get_function_sig` was made private.

`CachedProgram.struct_registry` was removed; `get_struct` now reads `TypeDefinitionKind::Struct` entries directly from `cached.metadata.types`.

## What Is Not Working and What Remains

### `get_function` Still Parses Strings

`engine.rs` `get_function` takes a `&str`. The path through the VM reaches this via `execute_call` in `vm.rs`, which receives `function_name: &FunctionName` from the `CallBytecode` and `CallExternal` instructions but converts it to a string for the call. The `MetaFunction` instruction holds `function_name: String` and drives `execute_meta_function`, which also calls `get_function(&str)`.

The correct design is for the VM to call a typed `get_bytecode_function(&FunctionName)` method using the `FunctionName` already present in the instruction. `get_function(&str)` would then serve only user-registered functions (the `function_registry` HashMap), where a string key is natural. `MetaFunction` should carry a `FunctionName` instead of a `String`. Until this is done, `engine.rs` retains a fallback path that constructs a `FunctionName` by string parsing.

See `src/structured-agent/src/bytecode/instruction.rs` and `src/structured-agent/src/bytecode/vm.rs` for the relevant instruction definitions and dispatch loop.

### Deprecated Functions Remain in Use

The deprecated constructors `ModuleName::from_str`, `FunctionName::plain`, `FunctionName::impl_fn`, and `FunctionName::fn_name` still appear in test files (`bytecode/tests.rs`, `il_analysis/call_arity_test.rs`, `compiler/mod.rs` tests). The deprecated functions themselves still exist in `symbols.rs` and cannot be deleted until the test call sites are updated.

### `type_implements_trait` Uses Bare Name Comparison

`type_implements_trait` in `query.rs` checks `k.type_name.name == type_name && k.trait_name.name == trait_name` across the impl and param-binding maps. This is the same category of bug as the former `type_by_name` scan: it will silently match the wrong impl if two modules define types or traits with the same name. The fix follows the same pattern as the struct-field resolution work: pass `TypeName` and `TraitName` structs rather than bare strings.

### Import Path Relative-to-Module Semantics

`UseImport.module` is constructed from the `NonEmpty<String>` path in the use statement with `ModuleName::new(path.clone())`. In the current implementation this treats the path as absolute (e.g. `use other::greet` produces `module: ModuleName { segments: ["other"] }`). The discovery system confirms this interpretation: `path.first()` is always the top-level module resolved against the entry directory. However, whether multi-segment paths such as `use a::b::greet` should be resolved relative to the current module's position (`current_module.segments + ["a", "b"]`) has not been settled. The current code treats them as absolute.

### Cognitive Complexity

`rust-code-analysis-cli` identified 51 functions with cognitive complexity of 10 or above. The largest are `check_call` (53), `collect_function_signatures` (46), and `check_modules` (43). Some helpers were extracted during this session but the core elaboration and collection passes remain dense. The split into focused files (Stage 6) creates the seams needed for further decomposition, but the work has not been done.

### `alias_map` in `CheckContext`

`CheckContext` carries `alias_map: &HashMap<String, String>`, populated by `build_alias_map` in `query.rs`. This maps an explicit alias string to the bare imported name. It is unclear whether any code path still reads this field after the `AliasToQualified` migration; it may be dead and removable.

## Related Documents

- `docs/checker-architecture-roadmap.md` — the six-stage plan implemented in this session
- `docs/type-system-abstractions.md` — the query-based architecture that Stage 6 seams are designed to support
- `src/structured-agent/src/typecheck/` — the split typecheck module
- `src/structured-agent-runtime/src/symbols.rs` — `UseImport`, `ModuleName::new`, `FunctionName::parse`
- `src/structured-agent/src/bytecode/instruction.rs` — `CallBytecode`, `CallExternal`, `MetaFunction`
- `src/structured-agent/src/runtime/engine.rs` — `get_function`, `build_cached_program`, `CachedProgram`
