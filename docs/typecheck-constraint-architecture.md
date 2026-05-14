# Typecheck Constraint Architecture

The type checker cannot correctly support union types in its current form. The root cause is a single structural problem: unification runs in the elaboration pass, after constraint solving, with no connection to the solver. Union subtype checking requires the solver to own all type resolution. Until that changes, adding union types produces two divergent resolution paths — one in the solver, one in elaboration — that cannot be reconciled.

## The Blocker

Union subtype checking replaces the current equality test (`got == *expected`) with a relation: `Image ≤ Image | Audio`. When generics interact with unions — `fn foo<T>(x: T | String)` called with `Image` — the constraint `T | String ≥ Image` must be solved once, globally, so that every use site gets a consistent substitution. That requires the solver to own both unification of type variables and subtype checking of union members.

In the current architecture, `build_typed_call` in `elaboration.rs` creates a fresh `Unifier` per call site, runs unification against argument types, and calls `unifier.apply_subst(&sig.return_type)` to produce the concrete return type. The solver's `SolvedConstraints` contains resolved trait impl mappings but no type variable substitutions from call sites. If union subtype logic were added to the solver, elaboration would not see it. The two passes would each do partial type resolution independently, and their results would conflict.

## Current Architecture

The checker runs three sequential passes coordinated by Salsa.

The first pass, check/synthesise, is `check_program` (a `#[salsa::tracked]` function returning `()`). It calls `check_module`, which calls `synthesize::check_definition`, walking the AST, synthesising types, and emitting side-effects as Salsa accumulators: `TypeErrorAccumulator` for definite errors such as unknown variables, and `Constraint` for type bound and trait obligations. `Constraint` is defined in `solver.rs` and declared with `#[salsa::accumulator]`.

The second pass is `solve_constraints` (`#[salsa::tracked]`, returns `SolvedConstraints`). It reads accumulated `Constraint` values via `check_program::accumulated::<Constraint>(db, program)`, resolves `TypeBound` constraints by equality checking, and resolves `TraitImpl` and `TraitBound` constraints. The resulting `SolvedConstraints` carries resolved trait impl mappings and inherent impl mappings.

The third pass is `elaborate_metadata` (`#[salsa::tracked]`, returns `MetaData`). It calls `elaborate_function_def` — a plain function, not salsa-tracked — for each function. Inside `build_typed_call`, a fresh `Unifier` is created per call site, unification runs on each argument against its formal type, and `unifier.apply_subst` produces the concrete return type used to annotate the typed AST.

```
/dev/null/query-graph-current.txt#L1-22
check_program  (salsa::tracked, returns ())
  │
  ├── emits → Constraint (salsa::accumulator)
  └── emits → TypeErrorAccumulator (salsa::accumulator)
       │
       ▼
solve_constraints  (salsa::tracked, reads accumulated Constraints)
  │
  └── returns SolvedConstraints
       │  (trait impl mappings only — no call-site substitutions)
       │
       ▼
elaborate_metadata  (salsa::tracked)
  │
  └── elaborate_function_def  (plain function, not tracked)
       │
       └── build_typed_call
            │
            └── Unifier (per call site, independent of SolvedConstraints)
                 │
                 └── apply_subst → concrete return type
```

## Two Disconnected Resolution Paths

The solver's `check_type_bound` performs equality checking (`actual_type != bound_type`) for the subset of constraints emitted as `ConstraintKind::TypeBound`. This covers explicitly emitted type bounds but not the generic instantiation that `build_typed_call` handles. Call-site unification is entirely separate from the solver: the `Unifier` in elaboration sees no constraints, receives no solved substitutions, and writes no results back to `SolvedConstraints`.

The consequence is that there are two type resolution paths that never communicate. The solver resolves trait obligations; elaboration resolves type variables. Neither knows about the other's results. For trait bounds this happens to be sufficient, because trait resolution does not depend on the concrete type substitutions at a call site (or rather, the current system does not attempt to express that dependency). For union types it fails immediately, because checking whether a concrete argument satisfies a union-typed parameter requires both subtype reasoning and, where type variables are involved, unification — work that must be done together in one place.

## The Correct Architecture

The fix is to move the `Unifier` into the solver and route constraint emission through return values rather than Salsa accumulators.

The synthesise/check pass should return a `CheckResult` containing a `Vec<Constraint>` rather than pushing constraints into an accumulator as a side-effect. This is not merely a style preference. Salsa's early-exit optimisation — where a tracked query's dependents are not re-executed if the query's return value is unchanged — is impossible with accumulators. Any re-execution of the check pass re-emits all constraints, forcing the solver and all downstream queries to rerun even when the constraint set has not changed. A source edit that renames a variable without affecting any type would still invalidate the entire solver. Returning constraints as a value restores Salsa's normal memoisation behaviour: if the returned `CheckResult` is equal to the previously cached value, downstream queries short-circuit.

The synthesise pass already synthesises argument types as part of its existing work. It therefore has all the information needed to emit `Constraint::Unify { call_site, var, ty }` for each generic parameter at each call site, without running unification itself. The solver then collects these unify constraints alongside the existing trait constraints, runs the `Unifier` once over the collected set, and stores the resulting per-call-site substitutions in `SolvedConstraints`. Elaboration retrieves the substitution for a given call site by lookup rather than by recomputing it.

```
/dev/null/query-graph-target.txt#L1-24
check_module  (salsa::tracked, returns CheckResult { constraints, errors })
  │
  └── returns Vec<Constraint>
       │  (Unify, Subtype, TraitBound — all constraint forms)
       │
       ▼
solve_constraints  (salsa::tracked, reads CheckResult)
  │
  ├── runs Unifier on Unify constraints
  ├── checks subtypes (including union membership)
  └── resolves trait obligations
       │
       └── returns SolvedConstraints
            │  (call-site substitutions + trait impl mappings)
            │
            ▼
elaborate_function_def  (salsa::tracked per function)
  │
  └── build_typed_call
       │
       └── looks up substitution from SolvedConstraints
            │
            └── applies pre-solved subst → concrete return type
```

With this structure, adding union subtype support means adding a `Constraint::Subtype` variant to the constraint language, emitting it from the synthesise pass when an argument is passed to a union-typed parameter, and discharging it in the solver by checking union membership. Elaboration is untouched.

## Salsa and Per-Function Tracking

`elaborate_function_def` is currently a plain function, which means `elaborate_metadata` re-elaborates every function on any change. Making `elaborate_function_def` a `#[salsa::tracked]` query means elaboration is cached per function and reruns only when that function's inputs change. Combined with early-exit on constraint generation — if the constraint set for a function does not change, the solver result for that function does not change, so elaboration does not rerun — incremental recompilation becomes fine-grained. Only the functions directly affected by a source change rerun their elaboration.

This matters in practice. The synthesise pass is the most expensive, and it currently drives all downstream work unconditionally. The accumulator model hides the cost by making it look like the solver is reading lazily, but the constraints are always re-emitted, so the solver always reruns. Per-function tracking with value-returning queries gives Salsa enough information to actually prune the dependency graph.

## The Solver Split

The solver as described has two distinct responsibilities that can be separated. Local solving is per-call-site unification: given the `Unify` constraints for one call site, produce a substitution. This work has no cross-function dependencies. Global solving is trait bound satisfaction, which requires scanning impl blocks across the whole program. Local solving could ultimately move into the synthesise pass itself — since each call site's unification is independent and its inputs are already available during synthesis — leaving only global constraints for the program-level solver. The type-system-abstractions document's HM(X) description maps directly onto this split: constraint generation in synthesis, local solving inlined or in a per-function query, global solving at program scope.

## Prototype

A working prototype demonstrating this architecture is at `structured-agent/prototypes/constraint-solver/`. It implements a minimal algebra with `Float`, `Double`, generic functions, and named union aliases. The three passes have hard boundaries enforced by their type signatures:

- `generate(expr, fns, aliases) -> (Option<Type>, GenResult)` — emits constraints, no `Unifier`
- `solve(constraints, aliases) -> SolvedConstraints` — runs the `Unifier`, checks subtypes
- `elaborate(expr, fns, aliases, solved) -> Option<TypedExpr>` — applies pre-solved substitutions, no `Unifier`

The test `elaboration_does_not_run_unifier` makes the boundary explicit: `elaborate` has no `Unifier` import and no means to create one. The solver test `solver_expands_alias_for_subtype` confirms that union alias checking (a `Subtype` constraint where `to` is an alias expanding to `[Float, Double]`) is discharged in the solver, not in elaboration.

## See Also

- [type-system-abstractions.md](type-system-abstractions.md) — HM(X) architecture, query-based checker design, and the target composition of bidirectional checking with constraint solving
- `structured-agent/src/structured-agent-typecheck/src/synthesize.rs` — current check/synthesise pass and `Unifier`
- `structured-agent/src/structured-agent-typecheck/src/solver.rs` — `Constraint` accumulator, `SolvedConstraints`, `solve_constraints`
- `structured-agent/src/structured-agent-typecheck/src/elaboration.rs` — `build_typed_call` and per-call-site `Unifier`
- `structured-agent/src/structured-agent-typecheck/src/db.rs` — Salsa query definitions
- Vytiniotis, D., Jones, S. P., Schrijvers, T., Sulzmann, M. (2011). "OutsideIn(X): Modular Type Inference with Local Assumptions." Journal of Functional Programming. https://www.microsoft.com/en-us/research/publication/outsideinx-modular-type-inference-with-local-assumptions/
- Salsa query framework: https://github.com/salsa-rs/salsa
