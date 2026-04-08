# Salsa Query Layer and Collection Cleanup

## Background

The typechecker in [`src/typecheck/`](../../src/typecheck/) previously performed three logically distinct phases inside a single `check_modules` function with no explicit boundaries between them. The goal of this work was to introduce [salsa 0.26](https://github.com/salsa-rs/salsa) as a query layer and make those three phases explicit:

1. Symbol table population — index definitions without any type checking
2. Typechecking — check expressions using salsa cached queries against the symbol tables
3. Output materialisation — translate `MetaData<CheckerRefs>` to `MetaData<TypedRefs>`

## What Was Achieved

### Phase 1 is clean

`populate_symbol_tables` is now pure indexing. It walks every parsed module and registers function signatures, type definitions, trait definitions, impl entries, and module definitions into `self.metadata`. No types are resolved, no validation is performed. At the end of the phase it constructs a `SymbolTablesInput` and stores it on `self.symbol_tables`, making the symbol tables available to the salsa query layer.

Type resolution (`resolve_type`) and type-definition validation (`validate_type_with_params`) that previously lived in `collection.rs` were moved into [`src/typecheck/elaboration.rs`](../../src/typecheck/elaboration.rs). `collect_function_signatures` now returns `()` rather than `Result<(), TypeError>`.

### The salsa query layer

[`src/typecheck/db.rs`](../../src/typecheck/db.rs) defines the salsa database and a set of `#[salsa::tracked]` free functions: `lookup_function_def`, `lookup_type_def`, `lookup_trait_def`, `lookup_impl_exists`, `lookup_impl_def`, and `find_trait_for_impl_call`. Symbol lookups in [`src/typecheck/query.rs`](../../src/typecheck/query.rs) and parts of [`src/typecheck/elaboration.rs`](../../src/typecheck/elaboration.rs) now call these tracked functions rather than reading `self.metadata` directly.

`SymbolTablesInput` is a `#[salsa::input]` struct holding the five symbol maps wrapped in `ArcPtr<HashMap<...>>`. `ArcPtr<T>` provides pointer-equality semantics to satisfy salsa's `PartialEq + Eq` requirement without needing structural equality on the definition types inside the maps.

### Sig and module-param support removed

`register_param_sigs` constructed synthetic impl entries using a `__param__` sentinel module name and string manipulation to produce qualified names. It mixed indexing with code-generation concerns and produced entries the rest of the compiler did not handle correctly. It has been deleted, along with the five compiler tests that depended on it. The AST and parser still represent `sig` declarations and module headers; the typechecker ignores them until they can be reimplemented on top of the query foundation.

## What Was Not Achieved

The goal of making typechecking itself query-driven was not reached. `check_modules` now calls three named methods, but `elaborate_modules` mixes phase 2 and phase 3 responsibilities: it both runs expression checking and produces the typed AST that `materialize_metadata` consumes. The separation is nominal. Elaboration is still an imperative `TypeChecker` method, not a `#[salsa::tracked]` function. Modifying one module's source still re-elaborates all modules.

The salsa tracked functions added in this work are lookup helpers that sit beneath the existing imperative flow. They are the right groundwork, but bolting them underneath an unchanged elaboration pass is not the same as making elaboration query-driven. A reader of `check_modules` today sees three labelled steps, but the middle step has not been restructured to match its label.

## What Remains

### `elaborate_modules` needs to become a tracked query

The real stage 2 requires `check_single_module_expressions` to become a `#[salsa::tracked]` function taking `ParsedModuleInput` and returning a checked result. The blockers are:

- `check_definition` takes `&mut self` but does not mutate `self`. Removing that `&mut` is a prerequisite.
- `TypeError` and `typed_ast::Module` do not implement `Eq`, which salsa requires on tracked function return types.

Once those are resolved, `elaborate_modules` becomes a loop that drives per-module tracked calls, and re-elaboration of unchanged modules becomes free.

### `build_type_import_map` bypasses salsa

The `build_type_import_map` helper in `query.rs` still accepts an explicit `&MetaData<CheckerRefs>` argument and calls `metadata.type_def(...)` directly, bypassing `SymbolTablesInput`. This is inconsistent with the rest of the query layer.

### `collect_native_sigs` string round-trip

In [`src/typecheck/collection.rs`](../../src/typecheck/collection.rs), `collect_native_sigs` constructs a `FunctionName` by formatting `"{}::{}", module, name` and immediately splitting the result with `rsplit_once("::")`. The module and name are available directly; the string round-trip serves no purpose.