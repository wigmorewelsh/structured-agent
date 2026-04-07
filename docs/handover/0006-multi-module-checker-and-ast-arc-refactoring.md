# Multi-Module Checker and AST Arc Refactoring

## Original Goal

The previous session ended with a `TypeChecker` that still operated one module at a time. Each module was type-checked in isolation by calling `check_module_with_external_sigs`, which required the compiler to pre-build a `SigTable` via `collect_sigs` before the loop, then pass slices of that table into each individual check. The symbol table (`MetaData<CheckerRefs>`) was populated per-module and discarded after each call; only the typed AST was threaded forward. The stated next step was to give the checker a single entry point that accepted all modules at once, performed both passes internally, and returned a unified `MetaData` alongside the typed modules.

A secondary concern noted in the handover was that `CheckerAstRef` stored copied fields from AST nodes rather than references to the nodes themselves, making it impossible to navigate back to the full definition from a symbol table entry.

## What Was Done

### Multi-Module Entry Point (`check_modules`)

`TypeChecker` gained a public method `check_modules` that replaces the old orchestration loop in `compiler/mod.rs`:

```rust
pub fn check_modules(
    &mut self,
    modules: &[ParsedModule],
    native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
) -> Result<(HashMap<String, typed_ast::Module>, HashMap<String, FunctionKind>, MetaData<CheckerRefs>), TypeError>
```

It performs three sequential passes. The first iterates every module and, for each, calls `collect_native_sigs` (which processes `Definition::Use` against the native module registry), `build_visibility_for_module` (which populates the shared `ModuleVisibility` map), and `collect_function_signatures` (which populates `self.metadata` with functions, structs, traits, and now also signature definitions). The second pass calls `register_param_sigs` for every module after all function signatures are known, so that module-parameter aliases resolve correctly against other modules' already-loaded functions. The third pass calls `check_single_module_expressions` for each module, which builds the per-module alias maps, constructs the `CheckContext`, and produces the typed AST.

The result is that `SigTable`, `collect_sigs`, `sigs_visible_to_module`, and the free function `type_check_module` in `compiler/mod.rs` have all been deleted. `compiler/sigs.rs` no longer exists.

### Cross-Module Call Resolution Fix

The old single-module path stored functions imported via `use lib::greet` as `FunctionName { module: "", name: "greet" }` (an unqualified key), allowing `lookup_sig` to find them via a fallback. In the multi-module path, `greet` is stored under its proper qualified key `FunctionName { module: "lib", name: "greet" }`. Without further change, calling `greet()` in the entry module would produce "Unknown function" because `lookup_sig` only tried the current-module and empty-module keys.

The fix was to extend `lookup_sig` to consult `alias_to_qualified` before falling back:

```rust
if let Some(qualified) = ctx.alias_to_qualified.get(resolved) {
    let name = FunctionName::from_qualified_str(qualified);
    if let Some(sig) = self.get_function_sig(&name) {
        return Some(sig);
    }
}
```

`alias_to_qualified` is already built from `Use` statements and maps local names to their qualified forms, so this correctly resolves `greet` to `lib::greet` and finds the symbol in metadata.

### `SigTable` Replacement in Vtable Resolution

`resolve_vtables` in `compiler/wiring.rs` previously took a `&SigTable` to look up named signature definitions and concrete module exports. It now takes `&MetaData<CheckerRefs>`. Named signatures are found via `metadata.types` (entries with `TypeDefinitionKind::Signature`); concrete module exports are found via `metadata.functions` by matching `fname.module.to_string()` against the concrete module name.

Signature definitions are now collected into `metadata.types` during `collect_function_signatures`, having previously been ignored. Each `Definition::Signature` produces a `TypeDefinition` with `TypeDefinitionKind::Signature { entries }` and a `CheckerAstRef::Signature(Arc<Vec<SigFunction>>)`.

### Module Visibility Without `SigTable`

`CompiledProgram::with_module_visibility` still requires a `HashMap<String, bool>`. Rather than sourcing this from the deleted `SigTable`, the compiler now builds it via a small free function `build_module_visibility` that iterates the parsed modules directly, qualifying function names the same way `collect_sigs` did.

### Arc-Wrapping of AST Definition Variants

The three data-bearing variants of `ast::Definition` that correspond to top-level definitions were changed to carry `Arc`-wrapped values:

```rust
pub enum Definition {
    Function(Arc<Function>),
    ExternalFunction(Arc<ExternalFunction>),
    Struct(Arc<StructDefinition>),
    TraitImpl { ..., functions: Vec<Arc<Function>>, ... },
    // inline variants unchanged
}
```

The parser wraps values at construction time with `Arc::new`. All read-only match arms across the analysis passes, the checker, and the compiler are unchanged because `Arc<T>` derefs transparently to `T`. Sites that previously cloned the inner value when converting to `typed_ast` now write `(**f).clone()` to perform a deep clone through the Arc.

### `CheckerAstRef` Carries Full AST Nodes

`CheckerAstRef` was restructured to store references to complete AST nodes rather than extracted field copies:

```rust
pub enum CheckerAstRef {
    Function(Arc<crate::ast::Function>, FunctionKind),
    ExternalFn { params, return_type, type_params, kind },
    Struct(Arc<crate::ast::StructDefinition>),
    Trait(Arc<Vec<crate::ast::SigFunction>>),
    Impl,
    Module,
    Signature(Arc<Vec<SigFunction>>),
}
```

`Function` holds an `Arc` cloned directly from the `Definition::Function` arc, meaning the symbol table entry and the AST share the same allocation. `ExternalFn` is the synthetic variant used for functions that have no corresponding AST node: native module functions, module-parameter aliases, and external functions from other modules. `insert_fn` always creates `ExternalFn`; `collect_function_signatures` creates `Function` when processing `Definition::Function`.

The helpers `get_struct_fields`, `get_trait_functions`, and `get_sig_functions` now dereference through the stored `Arc` to extract the data they need, rather than reading from pre-copied fields.

### Extraction of Helper Methods in `check_modules`

The pass-one loop was previously a single method body with three levels of nesting. It was decomposed into named private methods: `collect_native_sigs`, `build_visibility_for_module`, `check_single_module_expressions`, and `check_definition`. `check_definition` replaces a duplicated match arm that existed in both the old `check_module_with_external_sigs` and the new `check_modules`.

### Dead Code Removal

The following were deleted:

`check_module_with_external_sigs` — only called from tests and from `check_module`, neither of which is production code.

`check_module` — a thin wrapper used by unit tests; tests now call `check_modules` with a single `ParsedModule` constructed inline or via a local helper.

`ExternalSig` and its `impl` block — carried cross-module function signatures for the old per-module path; no longer needed.

`typecheck::type_check_module` in `typecheck/mod.rs` — a public free function that only delegated to `check_module`.

`compiler/sigs.rs` in its entirety — `SigTable`, `collect_sigs`, `sigs_visible_to_module`.

Test helpers in `bytecode/tests.rs` and `typecheck/tests.rs` were updated to call `check_modules` directly.

## Relevant Files

- `src/structured-agent/src/typecheck/checker.rs` — `TypeChecker`, `CheckerRefs`, `CheckerAstRef`, `check_modules` and its extracted helpers
- `src/structured-agent/src/typecheck/mod.rs` — now exports only `TypeChecker` and `TypeError`
- `src/structured-agent/src/ast/mod.rs` — `Definition` enum with Arc-wrapped variants
- `src/structured-agent/src/compiler/mod.rs` — orchestration; uses `check_modules`, `build_module_visibility`
- `src/structured-agent/src/compiler/wiring.rs` — `resolve_vtables` using `MetaData<CheckerRefs>`
- `src/structured-agent-runtime/src/symbols.rs` — `MetaData<R>`, `CheckerRefs`, `SymbolQuery`

## What Is Not Done

`Trait` and `Signature` are still inline variants in `Definition` (holding `name`, `functions`, and `span` as plain fields) rather than Arc-wrapped named types. This inconsistency is harmless but untidy: a future step could introduce `AstTrait` and `AstSignature` structs and Arc-wrap them as was done for `Function` and `StructDefinition`.

`TraitImpl` functions are stored in `metadata.functions` under the `ExternalFn` variant rather than the `Function` variant. This is because `substitute_self_in_fn` synthesises a new `Function` value (substituting the concrete type name for `Self`), so there is no original Arc to clone. The full AST for a trait implementation method is therefore not directly reachable from the symbol table without walking the original `Definition::TraitImpl`.

Visibility is still a separate `HashMap<String, bool>` rather than a field on `FunctionDefinition`. The `Visibility` enum already exists on `ModuleDefinition` in `symbols.rs`. Adding it to `FunctionDefinition` would let the checker query visibility directly from `MetaData` and remove the separate map entirely.

The deprecated `FunctionName` constructors (`plain`, `impl_fn`, `from_qualified_str`, `fn_name`) remain in use throughout `checker.rs`, `compiler/mod.rs`, `compiler/wiring.rs`, and the bytecode tests. They generate deprecation warnings. Migrating to direct struct construction is straightforward but mechanical.

`ModuleName::from_str("")` is used in several places to represent "no module" for functions that have no natural home (synthetic sigs, external functions stored without a module prefix). This is fragile: an empty string is a valid module name in principle, and anything that formats a `FunctionName` with an empty module will produce a bare unqualified name rather than failing visibly. A dedicated `Option<ModuleName>` or a sentinel value would be cleaner.

The `runtime_type_to_ast` helper exists in two places: the private method on `TypeChecker` (for `check_modules`' native sig processing) and the now-deleted `pub(crate)` function that was in `compiler/mod.rs`. The checker's copy is the only one that matters, but moving it to `ast` or `runtime` would make it available without duplication if a third site ever needs it.