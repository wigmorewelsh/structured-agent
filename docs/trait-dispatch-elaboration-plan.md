# Trait Dispatch: Elaboration Plan

This document covers the implementation plan for chunk 3 of the trait checker work described in [trait-checker-design.md](trait-checker-design.md). Chunks 1 and 2 are complete: `Self` resolution is fixed, and the constraint pipeline now correctly validates `TraitImpl` and `TraitBound` constraints.

Chunk 3 connects the solved constraints to the elaborated typed AST, producing `MethodBinding::Late` for trait method dispatch inside generic functions. Chunk 4, covered at the end, wires `witness_ref` into the runtime symbols.

## What is Being Dropped

Unqualified calls to impl functions — `add(v1, v2)` where `add` is defined inside an `impl` block — are legacy syntax predating method call support. They are not the target of this work. The test `test_impl_function_call_resolves_to_qualified_name` tests this legacy path and should be deleted. Direct method calls (`v1.add(v2)`) are the supported syntax and already work via `elaborate_method_call` and `find_impl_fn`.

## The Dispatch Problem

Given:

```
trait Add {
    fn add(self: Self, other: Self): Self
}
impl Vec2: Add {
    fn add(self: Vec2, other: Vec2): Vec2 { return self }
}
fn combine<T: Add>(a: T, b: T): T {
    return a.add(b)
}
fn main(): Vec2 {
    let v = Vec2 { x: 1, y: 2 }
    return combine(v, v)
}
```

When `combine` is elaborated, `T` is not yet concrete. The method call `a.add(b)` cannot be statically resolved to `Vec2::impl[0]::add` at definition time. Instead, `combine` must receive the `Add` implementation as an implicit module argument, and `a.add(b)` must dispatch through it at runtime via `CallIndirect`.

At the call site `combine(v, v)` in `main`, the concrete type `Vec2` is known, the solver has recorded `(Vec2, Add) -> impl[0]` in `SolvedConstraints.impls`, and the implicit module argument can be constructed as a `ModuleInstance` expression and prepended to the call.

This is the modular implicits pattern described in [modular-implicits-dispatch.md](modular-implicits-dispatch.md). The infrastructure for it — `MethodBinding::Late`, `CallIndirect`, `LoadModule`, implicit module params in function signatures — already exists for the module header parameter feature. Trait dispatch reuses it.

## Three Changes Required

### 1. Implicit module parameters in generic function elaboration

File: `src/structured-agent-typecheck/src/elaboration.rs`, function `elaborate_function`.

When a function has type parameters with trait bounds, each distinct `(type_param_name, trait_path)` pair becomes an implicit module parameter prepended to the function's typed parameter list. For `fn combine<T: Add>(a: T, b: T): T`, this produces one implicit parameter with a generated name (e.g. `__T_Add`) and type `RT::Named(trait_path)`.

The binding ID for this parameter is allocated from the type environment and declared as a variable, exactly as module header parameters are handled today in `elaborate_function` (see the `for (name, module_type_path) in module_params` loop). The difference is that these implicit params are derived from the AST function's `type_params` rather than from `get_module_header_params`.

At this stage the elaborated function's parameter list begins with the implicit module params, followed by the regular parameters. The bytecode compiler already handles this correctly for module header params — it assigns each parameter a slot and records `binding_id_to_slot` entries for all of them.

### 2. Method dispatch through the implicit module param

File: `src/structured-agent-typecheck/src/elaboration.rs`, function `elaborate_method_call`.

Currently `elaborate_method_call` calls `find_impl_fn(db, struct_type_name, method, ctx.module_name)` to resolve the impl function path. This works for concrete types but fails for generic type variables because `struct_type_name` is a generic name like `T`, not a registered type.

The new logic: when `receiver_type` is `RT::Generic(param_name)`, look up whether the enclosing function has a trait bound for `param_name` by scanning the type environment for an implicit module param registered under the naming convention `__<param_name>_<trait_name>`. If found, the binding ID of that implicit param is known from the environment lookup. The impl function path is `DefinitionPath::for_impl_fn(&trait_path, method)` — a path that the VM resolves at runtime against the concrete module passed in for that binding. This produces `MethodBinding::Late(binding_id, impl_fn_path)`.

For concrete receiver types, the existing `find_impl_fn` path continues to be used and produces `MethodBinding::Early`.

To make the name lookup reliable, the implicit param naming convention must be consistent between elaboration of the function definition (step 1) and elaboration of method calls within the body (step 2). A helper `implicit_param_name(type_param: &str, trait_name: &str) -> String` in `elaboration.rs` encodes this.

### 3. Implicit argument injection at call sites

File: `src/structured-agent-typecheck/src/elaboration.rs`, function `elaborate_call`.

When `elaborate_call` resolves a function whose signature has type parameters with trait bounds, it must:

1. Call `solve_constraints(db, ctx.program)` to obtain `SolvedConstraints`. This is safe here because `elaborate_call` runs inside `elaborate_metadata` which runs after `check_program` completes — there is no Salsa cycle.

2. For each bounded type param in `sig.type_params`, use the unifier (already run over the arguments) to get the concrete type. Extract the `DefinitionPath` from `RT::Named(path)` or `RT::Parameterized(path, _)`.

3. For each trait bound on that param, resolve the bound name to a `DefinitionPath` via `resolve_type_in_module`. Call `impl_for_type_and_trait(&solved, &type_path, &trait_path)` to get `impl_path`.

4. Prepend a `typed_ast::Expression::ModuleInstance { path: impl_path, params: vec![], ty: RT::Named(impl_path.clone()), span }` for each implicit param, in the same order as the implicit params were added in step 1. These are inserted at the front of `all_args` before the regular typed arguments.

The helper functions `impls_for_trait` and `impl_for_type_and_trait` are plain functions (not salsa-tracked) that take `&SolvedConstraints` by reference and are defined alongside the other helpers in `db.rs`.

## Tests

Delete: `test_impl_function_call_resolves_to_qualified_name` in `src/structured-agent-typecheck/src/tests.rs`. It tests unqualified impl calls, which are no longer supported.

Add to `tests::typed_ast_tests`:

- A test where a generic function with a trait-bounded type param contains a method call on the bounded variable. The elaborated typed AST for that method call must carry `MethodBinding::Late(_, path)` where `path` ends with the trait method name.
- A test where the call site of such a generic function carries a `ModuleInstance` expression as its first argument, with `path` equal to the resolved impl key.

These two tests together verify the round-trip from definition to call site.

## Chunk 4: witness_ref

`TypeDefinitionKind::Trait` carries `witness_ref: R::Witness`. For `CheckerRefs`, `Witness = NoWitness`. For the runtime refs used after elaboration, `Witness` should be `HashMap<DefinitionPath, DefinitionPath>` mapping implementing type path to impl key path.

This is populated by `elaborate_metadata` in `db.rs`, which already iterates all impl definitions. For each `ImplDefinition` with a `trait_name`, after resolving both names to `DefinitionPath`, insert `type_path -> impl_key` into the `witness_ref` of the corresponding `TypeDefinitionKind::Trait` entry in the elaborated metadata.

The VM does not need `witness_ref` for dispatch (call sites carry the impl path directly via `ModuleInstance` or `MethodBinding::Late`). The field is an index for runtime introspection and future features that need to enumerate all implementations of a trait without access to the type checker.

## See Also

- [trait-checker-design.md](trait-checker-design.md) — overall pipeline design and current state
- [modular-implicits-dispatch.md](modular-implicits-dispatch.md) — `CallIndirect` and implicit module argument mechanics
- `src/structured-agent-typecheck/src/elaboration.rs` — `elaborate_function`, `elaborate_method_call`, `elaborate_call`
- `src/structured-agent-typecheck/src/solver.rs` — `SolvedConstraints.impls`
- `src/structured-agent-typecheck/src/db.rs` — `solve_constraints`, `elaborate_metadata`
