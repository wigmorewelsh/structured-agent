# Salsa Query Layer and Collection Cleanup

## Background

The typechecker in [`src/typecheck/`](../../src/typecheck/) previously performed three logically distinct phases inside a single `check_modules` function with no explicit boundaries between them. Symbol lookups went directly through `self.metadata`, which meant no caching and no foundation for incremental re-elaboration. This work introduced [salsa 0.26](https://github.com/salsa-rs/salsa) as the query layer, made the three phases explicit, and routed symbol lookups through tracked salsa queries.

## What Changed

### Three explicit phases in `check_modules`

`check_modules` now delegates to three named methods in sequence:

```rust
self.populate_symbol_tables(modules, native_modules)?;
let typed_modules = self.elaborate_modules(modules)?;
let typed_metadata = self.materialize_metadata(modules, &typed_modules);
```

**Phase 1 — `populate_symbol_tables`** is pure indexing. It walks every parsed module, registers function signatures, type definitions, trait definitions, impl entries, and module definitions into `self.metadata`. No types are resolved, no validation is performed. At the end of the phase it constructs a `SymbolTablesInput` and stores it on `self.symbol_tables`.

**Phase 2 — `elaborate_modules`** resolves and validates types, runs expression-level checking, and produces a `HashMap<String, typed_ast::Module>`. All symbol lookups in this phase go through the salsa tracked query functions rather than `self.metadata` directly.

**Phase 3 — `materialize_metadata`** translates `MetaData<CheckerRefs>` to `MetaData<TypedRefs>` by cross-referencing the typed modules produced in phase 2. It is a pure translation pass with no resolution logic.

### The salsa database

[`src/typecheck/db.rs`](../../src/typecheck/db.rs) defines the database trait and concrete type:

```rust
#[salsa::db]
pub(super) trait TypeCheckDatabase: salsa::Database {}

#[salsa::db]
#[derive(Default)]
pub(super) struct TypeCheckDb { storage: salsa::Storage<Self> }
```

`TypeCheckDb` lives on the `TypeChecker` struct alongside a `Option<SymbolTablesInput>` field. The `SymbolTablesInput` is `None` until `populate_symbol_tables` completes; every query helper panics if called before that point.

### `SymbolTablesInput` and `ArcPtr<T>`

`SymbolTablesInput` is a `#[salsa::input]` struct holding the five symbol tables (functions, types, traits, impls, modules) as `ArcPtr<HashMap<...>>` fields. Salsa requires `PartialEq + Eq + Hash` on input fields to detect whether a re-set input has changed. The definition types inside the hash maps do not implement structural equality, so `ArcPtr<T>` provides pointer-equality semantics:

```rust
impl<T> PartialEq for ArcPtr<T> {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}
```

This satisfies salsa's change-detection contract without requiring `Eq` on the underlying definition types.

### Tracked query functions

`db.rs` exposes a set of `#[salsa::tracked]` free functions — `lookup_function_def`, `lookup_type_def`, `lookup_trait_def`, `lookup_impl_exists`, `lookup_impl_def`, and `find_trait_for_impl_call` — each of which takes `SymbolTablesInput` and one or more interned keys. All symbol lookups in [`src/typecheck/query.rs`](../../src/typecheck/query.rs) and [`src/typecheck/elaboration.rs`](../../src/typecheck/elaboration.rs) go through these functions rather than indexing `self.metadata` directly.

The interned key types (`InternedFunctionName`, `InternedTypeName`, `InternedTraitName`, `InternedImplKey`, `InternedModuleName`, `InternedString`) were made possible by adding `Eq` and `Hash` derives to the corresponding symbol types in an earlier preparatory commit.

### Visibility and alias resolution through salsa

`check_visibility` and `build_alias_to_qualified` in `query.rs` both previously read `self.metadata.functions` directly. Both now intern a `FunctionName`, call `lookup_function_def`, and act on the result. This is the pattern used throughout: intern the key, call the tracked function, extract what is needed from the `ArcPtr`.

### Trait scan in `resolve_impl_call`

`resolve_impl_call` previously iterated `self.metadata.traits` to find which trait provided a given function name for a given receiver type. That scan is now `find_trait_for_impl_call`, a tracked query that performs the same iteration over `SymbolTablesInput` and caches the result.

### Type resolution and validation moved to elaboration

Type resolution (`resolve_type`) and type-definition validation that previously lived in `collection.rs` were moved into [`src/typecheck/elaboration.rs`](../../src/typecheck/elaboration.rs). Collection is now responsible only for inserting raw entries; elaboration is responsible for resolving and validating what those entries contain.

### Sig and module-param support removed

The typechecker previously contained `register_param_sigs`, which constructed synthetic impl entries using a `__param__` sentinel module name and `format!("{module}::{name}")` string manipulation to produce qualified names. This mixed indexing with code-generation concerns and produced entries the rest of the compiler did not handle correctly. The function and the tests that depended on it (vtable compiler tests, sig compiler tests) have been deleted. The AST and parser still represent `sig` declarations and module headers; the typechecker ignores them until they can be reimplemented cleanly on top of the query-based foundation.

## What Remains

### Stage 5: remove spurious `&mut self` from `check_definition`

`check_definition` in `elaboration.rs` takes `&mut self` but does not mutate `self`. Removing that `&mut` is the next step. It is a prerequisite for eventually annotating `check_single_module_expressions` as `#[salsa::tracked]`, which would give per-module incremental re-elaboration.

### Making elaboration tracked

Once `check_definition` no longer requires `&mut self`, the remaining blocker for a tracked `check_single_module_expressions` is that `TypeError` and `typed_ast::Module` do not implement `Eq`. Adding those derives, combined with the `&mut` removal, would allow salsa to skip re-elaborating a module when its inputs have not changed.

### `build_type_import_map` bypasses salsa

The `build_type_import_map` helper in `query.rs` still accepts an explicit `&MetaData<CheckerRefs>` argument and calls `metadata.type_def(...)` directly. It does not go through `SymbolTablesInput`. This is low-priority but inconsistent with the rest of the query layer.

### `collect_native_sigs` string round-trip

In [`src/typecheck/collection.rs`](../../src/typecheck/collection.rs), `collect_native_sigs` constructs a `FunctionName` by first formatting a qualified string with `format!("{}::{}", module, name)` and then splitting it with `rsplit_once("::")`. The module and name components are available individually before the format call, so the string round-trip is unnecessary. The `FunctionName` should be constructed directly from those components.

## Notes on `PrimitiveRefs`

`PrimitiveRefs` in [`src/typecheck/refs.rs`](../../src/typecheck/refs.rs) remains in use as the type parameter for `primitive_types: HashMap<TypeName, Arc<TypeDefinition<PrimitiveRefs>>>` on `TypeChecker`. This map seeds built-in types during `seed_builtin_types` and is consulted during `materialize_metadata`. It is not dead code, but it sits outside the salsa layer and is worth revisiting once tracked elaboration is in place.