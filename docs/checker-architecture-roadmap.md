# Checker Architecture Roadmap

## Direction

The metadata table is the authoritative source for all symbol information. The
typechecker's job is to produce `MetaData<TypedRefs>` — complete, with all
call-site substitutions already applied. The compiler's job is a single
translation step. The runtime holds the compiled metadata and dispatches
directly from it.

```
Source AST
   │
   ▼
TypeChecker ──────────────────────────────────────────────────┐
  pass 1: collect_function_signatures (MetaData<CheckerRefs>) │
  pass 2: register_param_sigs                                  │  Query cache
  pass 3: elaboration + inline substitution                    │  (future)
   │                                                           │
   ▼                                                           │
MetaData<TypedRefs>  ◄─────────────────────────────────────────┘
   │
   ▼
BytecodeCompiler::compile_metadata
   │
   ▼
MetaData<BytecodeRefs>
   │
   ▼
Runtime holds Arc<MetaData<BytecodeRefs>>
dispatch: metadata.function(name)?.body_ref
```

The checker itself evolves toward the query-based architecture described in
`type-system-abstractions.md`. Each module boundary created in Stage 6 is a
seam: `query.rs` intercepts for a future cache, `constraints.rs` isolates
unification for a future HM(X) solver, `elaboration.rs` separates check and
synthesise modes for bidirectional elaboration.

Each stage below leaves the project compiling cleanly with all tests passing.

---

## Stage 1 — Inline call-site substitution in the typechecker `[not started]`

`lower_typed_module` in `compiler/wiring.rs` rewrites call sites after type
checking by consulting a pre-built vtable string map. The typechecker already
owns `metadata.impls` populated with `ImplDefinition` entries for every module
parameter binding (Stage 6 of the symbol-table roadmap). The substitution
should happen during elaboration, not after it.

Add `module_params: &'a [ModuleParam]` to `CheckContext`. In `check_call`,
after `make_function_name` produces a resolved `FunctionName`, if
`resolved.module` matches any `module_params[i].name`, call
`self.metadata.impl_for(...)` using the `__param__` sentinel key and replace
`resolved.module` with `impl_def.module`. This is the same lookup that
`lower_expression` performs; it moves from post-elaboration mutation to
inline resolution.

In the compiler, remove the lowering loop from `compile()`, delete
`reconstruct_typed_modules`, `sync_lowered_to_metadata`, and
`lower_typed_module` together with `header_params` from `wiring.rs`. The
`wiring.rs` file is deleted entirely.

Affected files: `typecheck/checker.rs`, `compiler/mod.rs`,
`compiler/wiring.rs` (deleted).

Verification: the four vtable tests (`test_vtable_populated_from_named_sig`,
`test_vtable_driven_by_explicit_binding`, `test_vtable_substitution_end_to_end`,
`test_vtable_dispatch_end_to_end`) continue to pass; `wiring.rs` no longer
exists; `reconstruct_typed_modules` and `sync_lowered_to_metadata` are deleted.

---

## Stage 2 — `emit_module` replaced by metadata reads `[not started]`

`emit_module` extracts external functions, struct field data, sig definitions,
and use aliases from `typed_ast::Module`. Three of these four are already
present in `MetaData<BytecodeRefs>`:

- **External functions**: `metadata.functions` entries where
  `ast_ref = TypedCheckerAstRef::Other(CheckerAstRef::ExternalFn { .. })`.
- **Struct definitions**: `metadata.types` entries with
  `kind = TypeDefinitionKind::Struct { fields }`.
- **Sig definitions**: `metadata.types` entries with
  `kind = TypeDefinitionKind::Signature { entries }`.

Use aliases are the outlier. Add `pub use_aliases: Vec<(String, String)>` to
`ModuleDefinition` in `symbols.rs`. Populate it during `collect_function_signatures`
pass 1 when a `Definition::Use` is processed, so the alias map travels with the
module rather than being extracted from the typed AST later.

With these in place, delete `emit_module`, `ModuleArtifact`, and the `merge`
call from `compile()`. Remove `external_functions`, `struct_definitions`, and
`sig_definitions` from `CompiledProgram`; expose them via `SymbolQuery` reads
on `compiled.metadata`. The runtime reads struct fields from
`metadata.type_by_name(name)?.kind` and external functions from
`metadata.all_functions()` filtered by the `ExternalFn` variant.

`CompiledProgram` reduces to: `metadata: MetaData<BytecodeRefs>`,
`use_aliases`, `main_function`, `source_path`, `module_visibility`.

The compiler pipeline becomes: parse → typecheck (`MetaData<TypedRefs>`) →
`compile_metadata` (`MetaData<BytecodeRefs>`) → scan for main.

Affected files: `structured-agent-runtime/src/symbols.rs` (`ModuleDefinition`),
`typecheck/checker.rs` (store aliases in pass 1), `compiler/mod.rs` (delete
`emit_module`, `ModuleArtifact`, simplify `CompiledProgram`),
`runtime/engine.rs` (read struct fields from metadata).

Verification: `test_compile_project_sig_stored`, `test_compile_project_two_files`,
and all struct-literal VM tests pass; `emit_module` and `ModuleArtifact` are deleted.

---

## Stage 3 — Runtime holds `Arc<MetaData<BytecodeRefs>>` `[not started]`

`run_with_handle` currently re-parses, re-type-checks, and recompiles on every
call. It then rebuilds a `function_registry: HashMap<String, Arc<dyn ExecutableFunction>>`
from the compiled metadata. Both are avoidable.

Add `compiled: Option<Arc<MetaData<BytecodeRefs>>>` to the `Runtime` struct.
Compilation happens once (lazily on first run, or eagerly on `build()`). On
subsequent calls, `run_with_handle` uses the cached metadata.

Remove the `function_registry` HashMap rebuild loop from `run_with_handle`.
Dispatch directly: `metadata.function(&name)?.body_ref` produces the
`BytecodeRef`; construct `BytecodeFunctionExpr` at call time. Use aliases are
resolved via `metadata.module(&module_name)?.use_aliases`.

`struct_registry` population also moves to compile time: iterate
`metadata.all_types()`, filter for `TypeDefinitionKind::Struct`, convert
`FieldDefinition` entries to `(String, Type)` once and cache.

`CompiledProgram` is consumed by the runtime and its `metadata` is stored as
`Arc<MetaData<BytecodeRefs>>`. The type `CompiledProgram` is either deleted or
reduced to a thin construction helper.

Affected files: `runtime/engine.rs`, `compiler/mod.rs`.

Verification: all integration tests and VM tests pass; the `function_registry`
rebuild loop is absent from `run_with_handle`; rerunning the same `Runtime`
without recompiling produces the same result.

---

## Stage 4 — Function signatures as `TypeDefinition` entries `[not started]`

`TypeDefinitionKind::Function { parameters, generic_parameters, return_type }`
exists in the runtime but is never populated. `FunctionDefinition.type_name`
currently holds only the return type, making it impossible to answer "what is
the full type of this function?" via the types collection alone.

During `collect_function_signatures` and `insert_fn`, after registering a
`FunctionDefinition`, also register a `TypeDefinition` with
`kind = TypeDefinitionKind::Function { parameters, generic_parameters, return_type }`
keyed by a `TypeName { name: fn_key.name.clone(), module: fn_key.module.clone() }`.
`FunctionDefinition.type_name` becomes a genuine foreign key into
`metadata.types`.

With this in place, "what is the return type of function X?" and "what are the
parameters of function X?" are answered by a single `type_def` query rather
than by pattern-matching the `ast_ref`. This is the precondition for the
checker's type queries going through `SymbolQuery` without carrying AST
references into the query results.

Add tests asserting that `metadata.type_def(fn_def.type_name)` returns
`TypeDefinitionKind::Function` for registered bytecode and external functions,
and that the parameters and return type match the source declaration.

Affected files: `typecheck/checker.rs` (`collect_function_signatures`,
`insert_fn`).

Verification: all existing tests pass; new tests confirm `type_def` queries on
function type names return `TypeDefinitionKind::Function`.

---

## Stage 5 — Remove non-AST-ref variants from `CheckerAstRef` `[not started]`

`Builtin` and `ModuleParamBinding` in `CheckerAstRef` are semantic markers, not
references to AST nodes. Their presence forces every match arm on `CheckerAstRef`
and `TypedCheckerAstRef` to handle cases that carry no node data.

Add `pub struct NoAst;` to `symbols.rs` and implement `AstRef` for it. Use it
as the `ast_ref` type for entries that have no AST node to point at:

- **`Builtin`**: expressed entirely by `TypeDefinitionKind::Primitive`. Replace
  `ast_ref: CheckerAstRef::Builtin` with `ast_ref: NoAst` in
  `seed_builtin_types`. The `CheckerAstRef::Builtin` variant is deleted.
  `CheckerAstRef` can no longer be the `Ast` associated type for entries that
  use `NoAst`; those entries are `TypeDefinition<SomeRefsWithNoAst>` rather than
  `TypeDefinition<CheckerRefs>`. The simplest transition is a dedicated
  `PrimitiveRefs` or reuse of a two-field `References` implementation with
  `Ast = NoAst`.

- **`ModuleParamBinding`**: the `ImplDefinition` record is the complete fact —
  `key` carries the param name and sig, `module` carries the concrete target.
  Replace `ast_ref: CheckerAstRef::ModuleParamBinding` with `ast_ref: NoAst`
  in the `ModuleBinding` insertion and `register_param_sigs`. Remove the
  `ModuleParamBinding` variant from `CheckerAstRef`.

All match arms on `CheckerAstRef` lose their `Builtin` and `ModuleParamBinding`
arms. `TypedCheckerAstRef::Other` wraps only genuine AST references.

Affected files: `structured-agent-runtime/src/symbols.rs`, `typecheck/checker.rs`,
`compiler/mod.rs` (if any match arms remain from earlier stages).

Verification: `CheckerAstRef` contains only `Function`, `ImplFunction`,
`ExternalFn`, `Struct`, `Trait`, `Impl`, `Module`, `Signature` — every variant
holds an Arc to an AST node; all tests pass.

---

## Stage 6 — Split `checker.rs` toward the query-based architecture `[not started]`

`checker.rs` at 2000+ lines holds five distinct concerns. Splitting it into a
module directory creates the seams that the query-based architecture in
`type-system-abstractions.md` requires. The split is mechanical: no behaviour
changes, no API changes, only file boundaries.

```
typecheck/
  mod.rs           — TypeChecker struct, check_modules orchestration,
                     CheckContext, TypeEnvironment, FunctionSignature,
                     public API (check_modules, function_kinds)
  refs.rs          — CheckerAstRef, CheckerRefs, TypedRefs,
                     TypedCheckerAstRef, SourceLocation, NoBody, NoWitness,
                     FunctionKind
  collection.rs    — passes 1-2: seed_builtin_types, collect_native_sigs,
                     collect_function_signatures, register_param_sigs,
                     insert_fn, runtime_type_to_ast
                     (the "what symbols exist" layer; future home of
                     demand-driven symbol registration)
  query.rs         — get_function_sig, get_struct_fields, get_trait_functions,
                     get_sig_functions, check_visibility, lookup_sig,
                     make_function_name, resolve_impl_call,
                     build_alias_map, build_alias_to_qualified
                     (all SymbolQuery reads in one place; the intercept
                     point for a memoising cache wrapper — swap in a
                     caching impl of SymbolQuery here without touching
                     elaboration or collection)
  constraints.rs   — unify_type, apply_subst, validate_type_with_params,
                     resolve_type
                     (the unification layer; seam for HM(X) constraint
                     emission: unify_type(a, b) becomes
                     emit(Constraint::Unify(a, b)) and a solver runs over
                     the accumulated set)
  elaboration.rs   — check_function, check_statement, check_expression,
                     check_call, check_block, check_boolean_condition,
                     check_list_literal, check_select,
                     check_if_else_expression, check_struct_literal,
                     check_field_access, check_definition,
                     check_single_module_expressions, substitute_self,
                     substitute_self_in_fn
                     (bidirectional elaboration; check_expression currently
                     returns the synthesised type implicitly — making
                     check and synth modes explicit function signatures is
                     the next step toward the architecture in
                     type-system-abstractions.md)
```

Each boundary serves a future purpose:

- `query.rs` is where a `CachingSymbolQuery` wrapper intercepts all reads
  without modifying elaboration or collection. Once all checker reads go
  through this module, adding a Salsa-style cache is an implementation of
  `SymbolQuery`, not a refactor of the checker.
- `constraints.rs` is where `unify_type(a, b) -> Result<AstType, TypeError>`
  becomes `emit_constraint(Constraint::Unify(a, b)) -> ()` and a separate
  `solve() -> Result<Substitution, TypeError>` runs over the accumulated
  constraint set. Grade constraints and sig obligations land alongside
  unification constraints in the same emission site.
- `elaboration.rs` is where check mode (`check(expr, expected) -> ()`) and
  synth mode (`synth(expr) -> Type`) become explicit. Currently `check_expression`
  returns an `AstType` (implicit synth) and callers pass an expected type
  (implicit check). Making the mode a first-class parameter is the
  bidirectional elaboration interface.

Affected files: `typecheck/checker.rs` (deleted), `typecheck/mod.rs` (new),
`typecheck/refs.rs` (new), `typecheck/collection.rs` (new),
`typecheck/query.rs` (new), `typecheck/constraints.rs` (new),
`typecheck/elaboration.rs` (new). All other files that import from
`typecheck::checker` update their import paths.

Verification: `checker.rs` is deleted; the six new files together compile to
the same public API; all tests pass without modification to test assertions.