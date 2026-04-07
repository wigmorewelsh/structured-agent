# Symbol Tables and TypeChecker Metadata

## Original Goal

The TypeChecker maintained four private HashMaps as internal state — one for function signatures, one for struct field definitions, one for trait definitions, and one for trait implementations. These were populated during a signature-collection pass and then queried during expression checking, but they were opaque to the rest of the system. The compiler separately ran `collect_sigs` to build a `SigTable` before handing data to the TypeChecker, duplicating much of the same information in a different shape. Nothing outside the checker or compiler could query what the system knew about any given function, type, or trait.

The goal was to define a proper symbol table — `MetaData<R>` in `structured-agent-runtime` — that would serve as the primary data structure for the type checker, the compiler, and eventually the runtime/VM, replacing the ad-hoc internal maps and the `SigTable`. The checker was to emit this metadata alongside the typed AST, and the checker's internal state was to be backed by it rather than by separate maps.

## What Was Done

### Symbol Table Definition (`symbols.rs`)

A new module was written at `src/structured-agent-runtime/src/symbols.rs`. It defines the full symbol table model in terms of generic reference parameters, so the same types can be used at compile time (carrying AST data), at link time (carrying bytecode offsets), or at runtime (carrying function pointers), without coupling those concerns to each other.

The central abstraction is the `References` trait:

```rust
pub trait References {
    type Source: SourceRef;
    type Ast: AstRef;
    type Body: BodyRef;
    type Witness: WitnessRef;
}
```

Each of the four associated types has a marker trait. Their intended meanings are:

- `SourceRef` — a link back to the original source file and span, for diagnostics and tooling.
- `AstRef` — a link to the AST or typed-AST node, so the symbol can carry its definition structure.
- `BodyRef` — a link to compiled bytecode or a native function entry point, populated after compilation.
- `WitnessRef` — evidence that a trait has been implemented; lives on the `TraitDefinition`, not the `ImplDefinition`.

`MetaData<R>` is the concrete store, holding five maps keyed by the various name types:

```rust
pub struct MetaData<R: References> {
    pub modules:   HashMap<ModuleName,  Arc<ModuleDefinition<R>>>,
    pub functions: HashMap<FunctionName, Arc<FunctionDefinition<R>>>,
    pub types:     HashMap<TypeName,    Arc<TypeDefinition<R>>>,
    pub traits:    HashMap<TraitName,   Arc<TraitDefinition<R>>>,
    pub impls:     HashMap<ImplKey,     Arc<ImplDefinition<R>>>,
}
```

`SymbolQuery` is a trait that can be implemented by anything acting as a symbol store, not just `MetaData` directly:

```rust
pub trait SymbolQuery {
    type Refs: References;
    fn function(&self, name: &FunctionName) -> Option<Arc<FunctionDefinition<Self::Refs>>>;
    fn type_def(&self, name: &TypeName) -> Option<Arc<TypeDefinition<Self::Refs>>>;
    fn trait_def(&self, name: &TraitName) -> Option<Arc<TraitDefinition<Self::Refs>>>;
    fn impl_for(&self, type_name: &TypeName, trait_name: &TraitName) -> Option<Arc<ImplDefinition<Self::Refs>>>;
    fn traits_implemented_by(&self, type_name: &TypeName) -> Vec<Arc<ImplDefinition<Self::Refs>>>;
}
```

`FunctionName` now carries structured data rather than being a raw string. It holds a `ModuleName` (a non-empty list of path segments), the function's own name, and a `FunctionNameKind` that distinguishes plain functions from trait-impl functions:

```rust
pub enum FunctionNameKind {
    Function,
    Impl { type_name: TypeName, trait_name: TraitName },
}
```

The old string-based constructors (`plain`, `impl_fn`, `from_qualified_str`, `fn_name`) have been retained on `FunctionName` as `#[deprecated]` methods so that existing call sites still compile while the migration to structured constructors proceeds. There are currently many such call sites throughout the compiler and bytecode modules.

### `CheckerRefs` — The Checker's Concrete Reference Types

Four local newtypes satisfy the orphan rule and implement the marker traits:

```rust
pub struct SourceLocation(pub FileId, pub Span);
pub struct NoBody;
pub struct NoWitness;

pub enum CheckerAstRef {
    Function { params: Vec<Parameter>, return_type: AstType, type_params: Vec<TypeParam>, kind: FunctionKind },
    Struct   { fields: Vec<(String, AstType)> },
    Trait    { functions: Vec<SigFunction> },
    Impl,
    Module,
    Signature,
}
```

`CheckerAstRef` replaced the earlier `AstSpan(Span)`, which stored only a source location. However, the current design is still wrong in a different way. Each variant stores *extracted field copies* — parameter lists, return types, field names — rather than a reference or pointer to the actual AST node. A `Function` variant holds a `Vec<Parameter>` cloned out of the original `ast::Function`; it does not point at the function itself.

This matters because the `AstRef` associated type is intended to let a consumer navigate back to the full AST node from a symbol table entry. With field copies, that navigation is impossible. Any deeper query — traversing a function body, inspecting attributes, performing analysis over the full definition — requires re-scanning the original AST rather than following a pointer from the symbol. The correct form would store `Arc<ast::Function>`, `Arc<ast::StructDefinition>`, and so on, so that the full node is always reachable:

```rust
pub enum CheckerAstRef {
    Function(Arc<crate::ast::Function>),
    Struct(Arc<crate::ast::StructDefinition>),
    Trait(Arc<Vec<crate::ast::SigFunction>>),
    Impl,
    Module,
    Signature(Arc<Vec<crate::ast::SigFunction>>),
}
```

The helper methods `get_function_sig`, `get_struct_fields`, and `get_trait_functions` would then extract what they need from the referenced node rather than from copied fields. This change is a prerequisite before the symbol table can support anything beyond the narrow queries the checker currently makes.

### TypeChecker Internals Replaced by `MetaData<CheckerRefs>`

The four internal HashMaps that previously held the checker's state:

```rust
function_signatures: HashMap<FunctionName, FunctionSignature>,
struct_definitions:  HashMap<String, Vec<(String, AstType)>>,
trait_definitions:   HashMap<String, Vec<SigFunction>>,
trait_impls:         HashMap<String, HashSet<String>>,
```

have been removed and replaced with a single field:

```rust
pub struct TypeChecker {
    metadata: MetaData<CheckerRefs>,
}
```

Five private helpers on `TypeChecker` mediate access:

- `insert_fn` — populates `metadata.functions` with a `CheckerAstRef::Function` carrying the full signature.
- `get_function_sig` — extracts a `FunctionSignature` on demand from the `CheckerAstRef`.
- `get_struct_fields` — finds a struct type by unqualified name and returns its field list.
- `get_trait_functions` — finds a trait by unqualified name and returns its function list.
- `type_implements_trait` — checks `metadata.impls` for an `ImplKey` matching both names.

The metadata is built incrementally during `collect_function_signatures`, not as a post-processing step. The previous `build_metadata` method has been deleted. At the end of `check_module_with_external_sigs`, the metadata is moved out via `std::mem::take` and returned as the third element of the result tuple. Note that until `CheckerAstRef` stores real AST pointers rather than field copies, the metadata is only useful for the specific surface queries the checker already makes; it cannot support general AST traversal from a symbol entry.

### `ParsedModule` Moved to `crate::ast`

`ParsedModule` was defined in `compiler/discovery.rs` with `pub(crate)` visibility. This prevented `typecheck/checker.rs` from accepting it as a parameter without introducing a circular module dependency. It has been moved to `src/structured-agent/src/ast/mod.rs` as a `pub` type. All existing import sites in `compiler/sigs.rs`, `compiler/wiring.rs`, and `compiler/mod.rs` have been updated accordingly.

## What Is Not Done

The TypeChecker still processes one module at a time. The calling code in `compiler/mod.rs` runs `collect_sigs` to build a `SigTable`, then calls `type_check_module` for each module in a loop, creating a fresh `TypeChecker` per module. The `SigTable`, `collect_sigs`, and `sigs_visible_to_module` in `compiler/sigs.rs` are therefore still in use and have not been removed.

The intended next step is to give `TypeChecker` a method that accepts `&[ParsedModule]` plus native module registrations, performs the two passes internally — first collecting signatures from all modules into the shared `MetaData`, then checking expressions across all modules against that unified table — and returns a map of typed modules alongside the populated `MetaData`. Once that exists, `SigTable` and `collect_sigs` can be deleted, and `type_check_module` in `compiler/mod.rs` can be replaced with a single call.

`resolve_vtables` in `compiler/wiring.rs` still takes a `&SigTable`. It uses `sig_table.sig_definitions` to enumerate the functions belonging to a named signature, and `sig_table.external_sigs` to enumerate the functions exported by a concrete module. Both of these are present in `MetaData<CheckerRefs>` — signature definitions as `TypeDefinitionKind::Signature` entries in `metadata.types`, and module functions in `metadata.functions`. The wiring module needs to be updated to query `MetaData` instead of `SigTable`. The user has indicated that the metadata tables will eventually replace the vtable mechanism entirely.

Visibility is currently tracked in a separate `ModuleVisibility` map (a `HashMap<String, bool>`) built by `collect_sigs` and threaded through as `ctx.module_visibility` inside the checker. There is no visibility field on `FunctionDefinition` or `ModuleDefinition` in the symbol table. The `Visibility` enum (`Public`/`Private`) exists on `ModuleDefinition` already; adding it to `FunctionDefinition` would allow the checker to drop the separate map and query visibility directly from the metadata.

## Dead Code and Known Issues

`ExternalSig` and the `check_module_with_external_sigs` signature remain in use. Once the multi-module checker method exists, `ExternalSig` becomes a transitional type with no long-term role; the information it carries belongs in `MetaData`.

All call sites for the deprecated `FunctionName` constructors — `plain`, `impl_fn`, `from_qualified_str`, `fn_name` — generate deprecation warnings. They are spread across `checker.rs`, `compiler/mod.rs`, `compiler/wiring.rs`, and the bytecode tests. These should be migrated to constructing `FunctionName` directly using its public fields and `ModuleName::from_str`. The `fn_name()` accessor can be replaced with `.name` directly.

`sigs.rs` will become dead code once the TypeChecker handles cross-module sig collection internally. Its tests document the current expected behaviour of signature visibility and should either be migrated to integration-level tests or preserved as unit tests of whatever replaces `collect_sigs`.

## Relevant Files

- `src/structured-agent-runtime/src/symbols.rs` — the symbol table model
- `src/structured-agent/src/typecheck/checker.rs` — `CheckerRefs`, `CheckerAstRef`, `TypeChecker`
- `src/structured-agent/src/ast/mod.rs` — `ParsedModule` now lives here
- `src/structured-agent/src/compiler/sigs.rs` — `SigTable`, `collect_sigs`; to be deleted
- `src/structured-agent/src/compiler/wiring.rs` — `resolve_vtables`; depends on `SigTable`
- `src/structured-agent/src/compiler/mod.rs` — orchestration; still uses `SigTable` loop