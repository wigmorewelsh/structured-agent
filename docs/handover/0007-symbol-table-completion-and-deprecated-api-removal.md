# Symbol Table Completion and Deprecated API Removal

## Original Goal

The previous session left several items unfinished. The `MetaData<CheckerRefs>` symbol table had been given a multi-module entry point and Arc-wrapped AST references for functions and structs, but three gaps remained. First, `CheckerAstRef::Impl` and `CheckerAstRef::Module` were unit variants carrying no data, making it impossible to navigate from a symbol table entry back to the AST for trait implementations or modules. Second, `metadata.modules` was never populated at all — the collection existed but `check_modules` never inserted anything into it. Third, the deprecated `FunctionName` constructors (`plain`, `impl_fn`, `from_qualified_str`, `.fn_name()`) were still in widespread use across the codebase, producing eleven build warnings. Primitive types (`Int`, `String`, `Boolean`, `Unit`, `List`, `Option`) had no entries in `metadata.types`, which meant any query-based consumer of the symbol table would find `type_def` returning `None` for every built-in type.

## What Was Done

### Deprecated `FunctionName` Constructors Removed

`FunctionName::plain`, `FunctionName::impl_fn`, `FunctionName::from_qualified_str`, and `.fn_name()` were replaced throughout the codebase with direct struct construction. The replacements are mechanical: `plain(module, name)` becomes a `FunctionName { name, module: ModuleName::from_str(module), kind: FunctionNameKind::Function }` literal; `impl_fn` is inlined with the `FunctionNameKind::Impl { type_name, trait_name }` body it previously hid; `from_qualified_str` is replaced with a `rsplit_once("::")` match at each call site; `.fn_name()` is replaced with `.name`. All `#[allow(deprecated)]` suppressions were removed at the same time.

The affected files were `typecheck/checker.rs`, `typecheck/tests.rs`, `compiler/mod.rs`, `compiler/wiring.rs`, `bytecode/compiler.rs`, `bytecode/tests.rs`, `runtime/engine.rs`, the nine `il_analysis` test files, and the integration test at `tests/integration/integration_test.rs`. The build now produces no deprecation warnings.

### `AstTraitImpl` and Arc-Wrapping of `Definition::TraitImpl`

`Definition::TraitImpl` was an inline-fields enum variant:

```rust
TraitImpl {
    type_name: String,
    trait_name: String,
    functions: Vec<Arc<Function>>,
    span: Span,
},
```

A named struct `AstTraitImpl` was introduced in `ast/mod.rs` and the variant was changed to a tuple form:

```rust
pub struct AstTraitImpl {
    pub type_name: String,
    pub trait_name: String,
    pub functions: Vec<Arc<Function>>,
    pub span: Span,
}

TraitImpl(Arc<AstTraitImpl>),
```

This mirrors the treatment already applied to `Function`, `ExternalFunction`, and `StructDefinition` in the previous session. The parser, the `Spanned` impl, the `Display` impl, both match arms in `checker.rs`, and the wildcard arm in `analysis/unused_return_values.rs` were updated accordingly.

### `CheckerAstRef::Impl` Now Carries the AST Node

With `AstTraitImpl` Arc-wrapped, `CheckerAstRef::Impl` was changed from a unit variant to one holding a reference:

```rust
Impl(Arc<crate::ast::AstTraitImpl>),
```

In `collect_function_signatures`, the `ImplDefinition` inserted into `metadata.impls` now receives `CheckerAstRef::Impl(Arc::clone(impl_arc))`, where `impl_arc` is the `Arc<AstTraitImpl>` destructured directly from `Definition::TraitImpl`. The symbol table entry and the AST node share the same allocation.

### `metadata.modules` Is Now Populated

`CheckerAstRef::Module` was changed from a unit variant to:

```rust
Module(Arc<crate::ast::Module>),
```

In the first pass of `check_modules`, after `collect_function_signatures` for each parsed module, a `ModuleDefinition` is inserted into `self.metadata.modules`:

```rust
let module_def = ModuleDefinition {
    name: ModuleName::from_str(effective_name),
    visibility: Visibility::Public,
    exports: vec![],
    source_ref: SourceLocation(parsed.file_id, Span::dummy()),
    ast_ref: CheckerAstRef::Module(Arc::new(parsed.module.clone())),
};
self.metadata.modules.insert(ModuleName::from_str(effective_name), Arc::new(module_def));
```

`SymbolQuery::module` will now return `Some` for any module that passed through `check_modules`.

### `SymbolQuery` Verified by Tests

The `SymbolQuery` trait is implemented generically for `MetaData<R>` in `structured_agent_runtime::symbols` and so was already available on `MetaData<CheckerRefs>`. However, no tests exercised it. A new `mod metadata_query_tests` block was added to `typecheck/tests.rs` with five tests covering each collection: `metadata_module_is_populated`, `metadata_function_is_queryable`, `metadata_struct_type_is_queryable`, `metadata_trait_is_queryable`, and `metadata_impl_is_queryable`. The last test also asserts that the `ast_ref` on the returned `ImplDefinition` matches `CheckerAstRef::Impl(_)`, confirming the Arc is carried through.

### Primitive Types Seeded in the Symbol Table

`TypeDefinitionKind::Primitive` was added to the enum in `structured_agent_runtime::symbols`:

```rust
pub enum TypeDefinitionKind {
    Struct { fields: Vec<FieldDefinition> },
    Function { parameters, generic_parameters, return_type },
    Signature { entries: Vec<SignatureEntry> },
    Primitive,
}
```

`CheckerAstRef::Builtin` (a unit variant) was added to `CheckerAstRef` to serve as the `ast_ref` for types that have no source AST node.

`TypeChecker::new()` now calls a private method `seed_builtin_types` that inserts six `TypeDefinition` entries into `self.metadata.types` before any module is checked:

| `TypeName.name` | `AstType` variant |
|---|---|
| `"()"` | `Unit` |
| `"Boolean"` | `Boolean` |
| `"String"` | `String` |
| `"Int"` | `Int` |
| `"List"` | `List(_)` |
| `"Option"` | `Option(_)` |

All six are keyed under `module: ModuleName::from_str("builtin")`, which matches the module string that `ast_type_to_type_name` assigns to all non-struct types. Three additional tests were added to `metadata_query_tests` to assert these entries are queryable.

## Relevant Files

- `src/structured-agent/src/ast/mod.rs` — `AstTraitImpl` struct, `Definition::TraitImpl(Arc<AstTraitImpl>)`
- `src/structured-agent/src/typecheck/checker.rs` — `CheckerAstRef`, `seed_builtin_types`, module population in `check_modules`
- `src/structured-agent/src/typecheck/tests.rs` — `mod metadata_query_tests`
- `src/structured-agent-runtime/src/symbols.rs` — `TypeDefinitionKind::Primitive`
- `src/structured-agent/src/compiler/parser.rs` — updated `parse_trait_impl` constructor and test
- `src/structured-agent/src/analysis/unused_return_values.rs` — updated wildcard arm
- `src/structured-agent/src/compiler/mod.rs` — deprecated constructor replacements
- `src/structured-agent/src/compiler/wiring.rs` — deprecated constructor replacements
- `src/structured-agent/tests/integration/integration_test.rs` — deprecated constructor replacements

## What Is Not Done

`Trait` and `Signature` in `ast::Definition` remain as inline-fields variants rather than Arc-wrapped named structs. The inconsistency noted in the previous handover is harmless but the pattern established for `Function`, `ExternalFunction`, `Struct`, and now `TraitImpl` has not been extended to these two variants.

`ModuleDefinition.exports` is always an empty `Vec`. The visibility and export surface of each module are already tracked separately in the `ModuleVisibility` hashmap built during `build_visibility_for_module`. Populating `exports` from that map would let `SymbolQuery` serve as the sole source of visibility information and allow the separate hashmap to be removed.

`ModuleDefinition.visibility` is hardcoded to `Visibility::Public` for every module. The `is_entry` flag on `ParsedModule` and the `is_pub` field on individual definitions are not consulted.

The `ast_type_to_type_name` function returns `TypeName { name: "List<Int>", module: "builtin" }` for an instantiated `List(Int)` type, because it calls `other.to_string()` and `List` renders as `"List<{inner}>"`. The seeded entry in `metadata.types` is keyed as `"List"` (bare name). There is therefore a mismatch: a function returning `List<Int>` has a `FunctionDefinition.type_name` that will not resolve against the seeded entry. Fixing this requires `ast_type_to_type_name` to strip type arguments for parameterized built-ins, or storing instantiated types separately.

`TraitImpl` functions are stored in `metadata.functions` under `CheckerAstRef::ExternalFn` rather than `CheckerAstRef::Function`. This is because `substitute_self_in_fn` synthesises a new `Function` value (replacing `Self` with the concrete type name), so there is no original `Arc<Function>` to store. The full source AST for a trait implementation method is reachable via the `ImplDefinition.ast_ref` (`CheckerAstRef::Impl`), but not directly from the `FunctionDefinition` for each method.

The checker's internal lookup helpers (`get_function_sig`, `get_struct_fields`, `get_trait_functions`, `get_sig_functions`) still access `self.metadata` fields directly rather than going through `SymbolQuery`. The expression-checking pass is therefore not driven by the query interface even though the data is fully available through it.

`ModuleName::from_str("")` remains in use as a sentinel for functions with no module home (synthetic sigs, external functions without a qualified prefix). This is fragile for the same reasons noted in the previous handover.

The `Generic(String)` variant of `AstType` represents a type parameter reference, not a named type, and is correctly absent from `metadata.types`. However, there is no validation that a `Generic` name used in a function signature is actually bound by a type parameter declaration on the enclosing function or struct.