# Sig Matching

## Goal

The goal of this work was to implement structural module-signature matching — the mechanism by which a concrete module is verified to satisfy a declared signature (sig) when it is passed as a module parameter. This is analogous to ML-style module systems where a module must satisfy a signature before it can be used in a functor position.

The language supports the following constructs:

```sa
sig Logger {
    fn log(msg: String): Unit
}

mod tasks(logger: Logger)

use tasks(concrete_logger)::run
```

Here `concrete_logger` must be verified to implement `Logger` before the program is accepted.

## What Was Done

### Collection

`register_type_definitions` in [`src/typecheck/collection.rs`](../src/structured-agent/src/typecheck/collection.rs) was extended to register `sig` definitions as types with kind `TypeDefinitionKind::Signature`. Each sig function entry stores the function name as a simple type reference (`AstType::simple(&f.name)`) rather than the return type, referencing the function type registered in the symbol table.

Modules are now also registered as types with kind `TypeDefinitionKind::Signature` via `register_module_as_type`. A module's entries are its public functions and all structs, each referenced by name. This allows a module to appear in a type position for sig matching.

`register_type_definitions` was refactored so that each definition kind is handled in its own method (`register_signature`, `register_struct`) rather than a single loop body.

`CheckerAstRef` in [`src/typecheck/refs.rs`](../src/structured-agent/src/typecheck/refs.rs) gained a `Signature(Arc<AstSignature>)` variant to carry the AST reference for registered sig types.

### Solver

`Constraint` in [`src/typecheck/solver.rs`](../src/structured-agent/src/typecheck/solver.rs) was refactored to carry a `ConstraintKind` enum rather than flat fields. The two variants are `TypeBound` (the original type-parameter bound checking) and `SigCheck { module_type, sig_type }`. The solver's `solve_constraints` delegates to `check_type_bound` and `check_sig_constraint`, each in their own function. `check_sig_constraint` further delegates to `check_sig_entries`.

The `SigCheck` infrastructure is in place but is not yet reachable — nothing in the pipeline emits a `SigCheck` constraint.

## What Is Not Working

### No SigCheck constraints are emitted

Nothing in the synthesis or checking passes emits a `SigCheck` constraint. `Definition::Use` is a no-op in `check_definition`, and `Definition::ModuleHeader` is filtered out before `check_definition` is ever called in `check_module`. The solver infrastructure exists but is never exercised.

### Sig entry checking is name-only

`check_sig_entries` in `solver.rs` checks only that each sig entry name is present in the module's entries. It does not compare parameter types or return types. A module with `fn log(x: Int): Unit` would satisfy a sig requiring `fn log(x: String): Bool`.

### Module param aliases are not resolved

Any approach to emitting sig constraints from use statements must handle the case where a name in the use path is a module param alias rather than a real module. The following cases all require alias resolution before sig checking can proceed.

A param used as a module with its own params:

```sa
mod somemodule(queue: io::Queue)
use queue(somebacking)::fifo
```

A param passed as an argument to another module:

```sa
mod somemodule(queue: io::Queue)
use innermodule(queue)::blah
```

A re-exported parameterised module as a single-segment use:

```sa
pub use moduleA(param_a)
```

Chained parameterised segments:

```sa
use moduleA(param_a)::moduleB(param_b)::somefunction
```

In all of these cases the concrete module or the sig type cannot be resolved without first substituting module param aliases. Critically, the codebase already solves this problem for functions and struct types. Queries such as `resolve_use_param_bindings`, `resolve_function_alias_via_param`, `resolve_type_alias`, and `module_exports_type` in `db.rs` already handle alias substitution, transitive re-exports, and module param binding for those cases. The sig constraint emission should be built on top of that existing machinery rather than re-implementing path traversal independently.

### Transitive re-exports are not traversed

The code made no attempt to follow re-exports when resolving use paths. Consider:

```sa
mod A(paramA) {
    mod B {
        mod C: SigC {
            fn do(): ()
        }
    }

    pub use B::C
}

use A(someMod)::C::do
```

Resolving `use A(someMod)::C::do` requires following `A`'s internal `pub use B::C` to understand that `C` resolves to `A::B::C`, and then checking that `someMod` satisfies `SigC`. The implementation only examined the top-level use statement and matched its segments directly against module header params. No transitive path resolution through re-exports was implemented. The existing `module_exports_type` and `module_exports_function` queries already perform exactly this traversal for types and functions, so sig checking should hook into that resolution rather than duplicate it.

### collect_module_exports includes Signature kinds without visibility filtering

`collect_module_exports` in `collection.rs` exports `TypeDefinitionKind::Signature` the same as structs, but `TypeDefinitionKind::Signature` now covers both explicitly declared sigs and every module registered as a type. This may produce unexpected exports.

### Sig entries for traits and sigs store function name rather than function type

Each `SignatureEntry` in a registered sig or trait stores `AstType::simple(&f.name)` as its `type_name`. This references a function type by its local name, which is only meaningful if that function is separately registered in the type table. For sigs and traits, the functions are not registered as separate type definitions, so the reference does not resolve. This was left as a known limitation to be addressed when full sig matching is implemented.

## Cleanup

`resolve_use_param_bindings` and `resolve_function_alias_via_param` in `db.rs` both contain inline logic for extracting `ModuleHeader` params from a module's AST. This pattern should be extracted into a shared query when the sig-checking work resumes.