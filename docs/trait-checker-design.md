# Trait Checker Design

Traits are partially implemented across the parser, type checker, and bytecode compiler, but the three phases of the checker pipeline — constraint emission, constraint solving, and elaboration — are not yet connected. The parser accepts trait declarations and impl blocks. The symbol tables register them. The solver never consults them. The elaborator never produces the implicit module argument that trait dispatch requires. This document describes the design that closes those gaps.

The broader context for trait dispatch — why SA uses modular implicits rather than monomorphisation or dictionary passing, how coherence is enforced, and how dispatch interacts with durable execution — is in [constrained-generics.md](constrained-generics.md) and [modular-implicits-dispatch.md](modular-implicits-dispatch.md).

## The Pipeline

The HM(X) architecture in [type-system-abstractions.md](type-system-abstractions.md) separates constraint generation from solving, and both from elaboration. For traits, each phase has a distinct responsibility:

```
/dev/null/pipeline.txt#L1-10
Source AST
   │
   ▼
Synthesise / check
  (impl declarations: resolve and emit TraitImpl constraints)
  (call sites: look up SolvedConstraints)
   │
   ▼
Constraint solver
  (look up matching impl by resolved DefinitionPath; verify completeness; emit errors)
   │
   ▼
Elaboration
  (query SolvedConstraints to build implicit module arguments)
```

Resolution occurs only in the synthesise phase, where module context is available. The solver and elaboration phases work entirely with `DefinitionPath` values. No name resolution occurs after constraint emission.

## Constraint Shape

The current `ConstraintKind::TypeBound` in `src/structured-agent-typecheck/src/solver.rs` carries `actual_type: Type` and `bound_type: Type`, and the solver checks `actual_type == bound_type`. This is structurally wrong for trait bounds: it compares the concrete argument type against the trait type rather than asking whether an impl exists. The constraint form must change.

The replacement is a `TraitImpl` variant on `ConstraintKind`:

```/dev/null/constraint.rs#L1-6
TraitImpl {
    type_path: DefinitionPath,
    trait_path: DefinitionPath,
    span: Span,
    file_id: usize,
}
```

Both paths are resolved at emission time when the impl declaration is walked. The solver checks completeness using these resolved paths and, if the impl is valid, records the `impl_path` in `SolvedConstraints`. Elaboration queries and call-site bound checks then read from `SolvedConstraints`.

`SolvedConstraints` in `solver.rs` currently holds only `resolved: HashMap<String, HashMap<String, Vec<Type>>>`, a structure oriented towards `TypeBound`. It needs an additional field recording the resolved impl mappings — `HashMap<(DefinitionPath, DefinitionPath), DefinitionPath>` mapping `(type_path, trait_path)` to `impl_path` — so that the elaboration queries have a typed, cached source of truth.

## Synthesise Phase

The synthesise phase has two distinct responsibilities for traits, handled at different points in the AST walk.

When walking an impl declaration, the module context is available and both the type name and trait name are in scope. This is the correct point to resolve each to a `DefinitionPath` via `resolve_type_in_module` and emit a `TraitImpl` constraint. The constraint fires once per impl declaration, not once per call site, and carries no information about call sites at all.

When walking a call site with bounded type parameters, `synthesize_call` resolves the concrete type from unification and the trait bound to `DefinitionPath`s, then looks them up in `SolvedConstraints`. No constraint is emitted at the call site.

`find_impl_fn` in `src/structured-agent-typecheck/src/db.rs` is the existing ad-hoc lookup used for method call resolution. It matches on raw `.name()` strings and restricts to the current module, making it incorrect for imported types. It should be removed. The method call resolution it currently serves should be replaced with proper `DefinitionPath`-based resolution via `SolvedConstraints`.

## Solver Phase

The solver receives `TraitImpl` constraints carrying a resolved `type_path` and `trait_path`. Its job is to find the matching impl and verify it is complete.

Finding the impl requires resolving the `type_name` and `trait_name` fields of each `ImplDefinition<CheckerRefs>` in the symbol table. These fields are `AstType` values — unresolved at registration time. Each `ImplDefinition` carries its own `module: DefinitionPath`, which is the correct context for resolving its names. The solver calls `resolve_type_in_module` — a `#[salsa::tracked]` query, callable from the solver — with the impl's own module as context, and compares the resulting `DefinitionPath` values against the constraint's `type_path` and `trait_path`. This is a `DefinitionPath` comparison throughout; there is no string name matching.

Two error variants defined in `src/structured-agent-runtime/src/error.rs` are currently dead code — defined but never emitted:

- `TraitImplMissingFunction` — a matching impl exists but omits a function the trait requires.
- `UnknownTrait` — an `impl` block names a trait that has no corresponding `trait` declaration in the symbol table.


When a matching impl is found and complete, the solver records the `impl_path` in `SolvedConstraints`. The four ignored tests in `src/structured-agent-typecheck/src/tests.rs` — `test_trait_bound_satisfied_for_int`, `test_trait_bound_not_satisfied_for_string`, `test_trait_impl_missing_function_is_error`, and `test_unknown_trait_in_impl_is_error` — express the exact behaviour this phase must produce and are the acceptance criteria.

## Elaboration Queries

With `SolvedConstraints` carrying a resolved `(type_path, trait_path) → impl_path` mapping, the elaboration queries are two `#[salsa::tracked]` functions that compose:

```/dev/null/queries.rs#L1-16
#[salsa::tracked]
fn impls_for_trait(
    db: &dyn TypeCheckDatabase,
    solved: SolvedConstraints,
    trait_path: InternedTraitName,
) -> Vec<DefinitionPath> { ... }

#[salsa::tracked]
fn impl_for_type_and_trait(
    db: &dyn TypeCheckDatabase,
    solved: SolvedConstraints,
    type_path: InternedTypeName,
    trait_path: InternedTraitName,
) -> Option<DefinitionPath> { ... }
```

`impl_for_type_and_trait` calls `impls_for_trait` and filters by type path. Both operate on `SolvedConstraints`, not the raw symbol table. Salsa memoises both; repeated calls for the same `(type_path, trait_path)` pair are free after the first.

The elaborator uses `impl_for_type_and_trait` when it encounters a call to a function with bounded type parameters. The concrete type is known at that point from the typed AST. The result is a `DefinitionPath` for the impl, which becomes a `ModuleInstance` expression prepended as the implicit argument. This is the point at which `MethodBinding::Late` — already defined in `src/structured-agent-typed-ast/src/lib.rs` — is produced for trait-bounded calls, enabling `CallIndirect` dispatch in the bytecode compiler. The bytecode path from `MethodBinding::Late` through `CallIndirect` to the VM's indirect dispatch is already implemented for module header parameters and requires no changes.

## Self Substitution

Trait method signatures use `Self` to refer to the implementing type: `fn add(self: Self, other: Self): Self`. In the current codebase `Self` is a raw string, not a bound type variable. When registering impl function signatures in the type checker, `Self` must be substituted with the concrete implementing type — `Vec2` for `impl Vec2: Add`. Without this substitution, `self: Self` in an impl body does not resolve to a concrete type, parameter type checking fails silently, and the typed AST carries an unresolvable generic name rather than a real struct type.

The substitution belongs in `register_impl_function` in `src/structured-agent-typecheck/src/collection.rs`, where the implementing type name is known. It is a textual substitution over the function's parameter and return type `AstType` values before they are registered in the symbol table.

## witness_ref

`TypeDefinitionKind::Trait` in `src/structured-agent-runtime/src/symbols.rs` carries a `witness_ref` field of type `R::Witness`. For `CheckerRefs`, `Witness = NoWitness`, and `register_trait` in `src/structured-agent-typecheck/src/collection.rs` always sets it to `NoWitness`. Nothing populates it. The field exists as a placeholder for a resolved impl dictionary reference but is currently dead.

Once the solver records resolved impl paths in `SolvedConstraints`, `witness_ref` can be populated during elaboration with the resolved `DefinitionPath` for the impl — or the field can be removed and the impl path derived entirely from `SolvedConstraints` at elaboration time. Either way, leaving it as a permanently unset placeholder is not the end state.

## Trait Type Parameters

`AstTrait` in `src/structured-agent-ast/src/ast/mod.rs` has no type parameters. `trait Mappable<A, B>` is not currently representable. This matters for higher-kinded or parameterised traits but is not required for monomorphic traits. The four ignored tests and the end-to-end dispatch mechanism do not depend on it. This is deferred.

## Current State

The symbol table infrastructure is in place. `MetaData.impls` stores `ImplDefinition` entries keyed by `DefinitionPath`. `register_trait` and `register_trait_impl` in `src/structured-agent-typecheck/src/collection.rs` populate those entries. `InternedTraitName` and `InternedTypeName` are defined in `src/structured-agent-typecheck/src/db.rs`. `MethodBinding::Late` and `Expression::ModuleInstance` exist in the typed AST. `CallIndirect` is implemented in the bytecode compiler and VM.

What is absent is the connection between the symbol table contents and the constraint pipeline. The synthesise phase emits a structurally incorrect constraint. The solver never consults the impl registry. The elaborator never produces `MethodBinding::Late` for trait-bounded calls. `TraitImplMissingFunction` and `UnknownTrait` exist but are unreachable. `Self` is never substituted. `witness_ref` is never set. `find_impl_fn` performs ad-hoc string matching where `DefinitionPath` resolution is required.

## See Also

- [constrained-generics.md](constrained-generics.md) — trait declarations, bounds, coherence, and dispatch strategy
- [modular-implicits-dispatch.md](modular-implicits-dispatch.md) — the bytecode dispatch mechanism and naming foundation
- [type-system-abstractions.md](type-system-abstractions.md) — the HM(X) pipeline and query-based architecture
- [module-system.md](module-system.md) — sigs, module parameters, and wiring
- `src/structured-agent-typecheck/src/solver.rs` — current constraint kinds and solver
- `src/structured-agent-typecheck/src/synthesize.rs` — constraint emission at call sites
- `src/structured-agent-typecheck/src/collection.rs` — trait and impl registration, Self substitution site
- `src/structured-agent-runtime/src/error.rs` — `TraitBoundNotSatisfied`, `TraitImplMissingFunction`, `UnknownTrait`
- `src/structured-agent-typed-ast/src/lib.rs` — `MethodBinding::Late`, `Expression::ModuleInstance`
- `src/structured-agent-runtime/src/symbols.rs` — `TypeDefinitionKind::Trait`, `witness_ref`
- Wadler, P. and Blott, S. (1989). "How to Make Ad-Hoc Polymorphism Less Ad Hoc." POPL 1989. https://dl.acm.org/doi/10.1145/75277.75283
- White, L., Bour, F., Yallop, J. (2015). "Modular Implicits." ML Workshop 2014. https://arxiv.org/abs/1512.01438
- Vytiniotis, D., Jones, S. P., Schrijvers, T., Sulzmann, M. (2011). "OutsideIn(X): Modular Type Inference with Local Assumptions." Journal of Functional Programming. https://www.microsoft.com/en-us/research/publication/outsideinx-modular-type-inference-with-local-assumptions/