# Generics Implementation

Parametric polymorphism — generics — allows a single function definition to operate over values of any type, with the concrete type resolved at each call site. This document describes the implementation of generics in SA, building directly on the typed AST pipeline described in [typed-ast-pipeline.md](typed-ast-pipeline.md).

The theoretical basis is the Hindley-Milner algorithm (Damas and Milner, "Principal type-schemes for functional programs", *POPL 1982*, https://dl.acm.org/doi/10.1145/582153.582176), though SA does not require full type inference. Type variables appear only in explicitly annotated function signatures — the programmer writes `fn head<T>(list: List<T>): Option<T>` — and are resolved by one-pass left-to-right unification at each call site. Pierce's *Types and Programming Languages* (MIT Press 2002, ch. 22) and Harper's *Practical Foundations of Programming Languages* (Cambridge 2016, ch. 47) provide the formal grounding for the unification and substitution steps.

## Prerequisites

All four phases of the typed AST pipeline are complete. The type checker produces a `TypedModule` in which every expression carries a `ty: ast::Type` and every call carries a `resolved: String` and `kind: FunctionKind`. The bytecode emitter reads from this structure directly and requires no side-channels. Generics are, from the emitter's perspective, a non-event: the type checker resolves `T = String` at the call site, writes `Type::String` into the `TypedExpression` node, and the emitter never encounters a type variable.

## Syntax

Generic functions are declared with a type parameter list immediately after the function name, following the same angle-bracket convention used for `List<T>` and `Option<T>` in type positions:

```/dev/null/example.sa#L1-3
fn head<T>(list: List<T>): Option<T> {
    ...
}
```

Type parameters are scoped to the function signature and body. A call site passes a concrete `List<String>` and the checker unifies `T = String`, yielding a return type of `Option<String>`. The `extern fn` declaration follows the same syntax, allowing native functions to declare themselves as generic.

## Design: Generic/Struct Disambiguation

The parser does not distinguish type variables from struct names. `parse_type()` emits `Type::Generic(name)` for any identifier that is not a built-in keyword (`List`, `Option`, `Boolean`, `String`, `Int`, `Unit`). The type checker then reclassifies names: `resolve_type` converts `Generic(name)` to `Struct(name)` when `name` is present in the struct registry. Names that survive as `Generic` after resolution are treated as type variables and validated against the enclosing function's `type_params`. This means `Type::Struct` is no longer produced by the parser — it is exclusively a checker-side classification.

## Edge Case: Type Variables in Return Position Only

When a type variable appears only in the return type and not in any parameter — `fn make_none<T>(): Option<T>` — unification over the argument list produces no bindings for `T`. The call succeeds and the return type is `Option<Generic("T")>`. This is permitted; the caller receives an underspecified type. This behaviour is documented in the test `test_generic_fn_type_var_only_in_return_type_call_succeeds_with_unresolved_generic`.

## Implementation Phases

### Phase G1: AST changes — done

Added `Generic(String)` to `ast::Type` and `type_params: Vec<String>` to `ast::Function`, `ast::SigFunction`, and `ast::ExternalFunction`. Match arms across `bytecode/compiler.rs` and `compiler/mod.rs` map `Generic(name)` to `Struct(name)` as a fallback in the conversion paths that follow type checking; these paths are only reached after unification has resolved all type variables and should never see a `Generic` variant in practice.

### Phase G2: Parser support — done

`parse_type()` in [`compiler/parser.rs`](../src/structured-agent/src/compiler/parser.rs) now emits `Type::Generic(name)` for all identifier types. An optional `<T, U, ...>` clause was added after the function name in `parse_function`, `parse_sig_function`, and `parse_external_function`. No lexer changes were required: `<` and `>` were already handled by `lex_char` for `List<T>` and `Option<T>`. A pre-existing backtracking bug was also fixed: `lex_string("Boolean")`, `lex_string("String")`, and `lex_string("Int")` were not wrapped in `attempt`, which caused single-letter type variables beginning with `B`, `S`, or `I` to fail without backtracking.

### Phase G3: Type checker — validation and unification — done

`FunctionSignature` gained a `type_params: Vec<String>` field (the previously present but unused `is_pub` field was removed at the same time). `validate_type` was extended to a `validate_type_with_params` variant that accepts the enclosing function's type params and permits `Generic(name)` only when the name appears in that list; unknown type variables in non-generic functions are now a `TypeError`. `check_call` builds a substitution map via `unify_type` for calls to generic targets, applies it to the return type via `apply_subst`, and writes the concrete types into the `TypedExpression`. Error messages name the concrete types after substitution, not the type variable names.

The two core functions added to [`typecheck/checker.rs`](../src/structured-agent/src/typecheck/checker.rs) are:

```/dev/null/checker.rs#L1-8
fn unify_type(formal: &AstType, actual: &AstType, subst: &mut HashMap<String, AstType>) -> bool
// Generic(name) in formal: bind or consistency-check.
// List/Option: recurse into inner types.
// All other cases: require formal == actual.

fn apply_subst(ty: &AstType, subst: &HashMap<String, AstType>) -> AstType
// Replace Generic(name) with its binding; recurse into List/Option.
```

### Phase G4: End-to-end tests — done

Tests added covering: a generic function called with a concrete type yielding the correct substituted return type; a two-type-parameter function; a type mismatch producing `ArgumentTypeMismatch` with concrete type names; a generic `extern fn`; the return-position-only edge case; and a generic call result used in a downstream type-sensitive context. Tests live in `src/typecheck/tests.rs` and `src/tests/integration_test.rs`.

### Phase G5: Final cleanup — done

All clippy warnings across the codebase were resolved, including pre-existing ones:

- Dead `BrTrue` variant and the associated `emit_brtrue`/`emit_drop` methods in `bytecode/builder.rs` were removed.
- The unused `is_pub` field on the private `FunctionSignature` struct was removed.
- CLI error variants were renamed to satisfy `enum_variant_names` (`IoError` → `Io`, etc.).
- A complex type annotation in `compiler/parser.rs` was simplified.
- The root cause of approximately fifty dead-code warnings — the binary crate re-declaring the library's entire module tree with `mod` instead of using `use structured_agent::...` — was fixed in `main.rs`.

## Future Work

Generic `sig` declarations — `sig fn map<T, U>(list: List<T>, ...): List<U>` — raise questions about how a sig's type parameters interact with the module parameter system and the `in M` constraint. These are deferred until the module system's Phase 6 (contract matching) is more settled. See [module-system-implementation.md](module-system-implementation.md).

Generic structs — `struct Pair<A, B>` — require changes to struct definition, instantiation, and field access that are orthogonal to function generics. They are deferred.

## See Also

- [typed-ast-pipeline.md](typed-ast-pipeline.md) — the pipeline this work builds on
- [module-system.md](module-system.md) — the module system, including `in M` and `deferred`
- [`src/structured-agent/src/ast/mod.rs`](../src/structured-agent/src/ast/mod.rs) — the untyped AST
- [`src/structured-agent/src/typecheck/checker.rs`](../src/structured-agent/src/typecheck/checker.rs) — the type checker
- [`src/structured-agent/src/compiler/parser.rs`](../src/structured-agent/src/compiler/parser.rs) — the parser
- Damas, L. and Milner, R. (1982). "Principal type-schemes for functional programs." *POPL 1982*. https://dl.acm.org/doi/10.1145/582153.582176
- Pierce, B. C. (2002). *Types and Programming Languages*. MIT Press. Chapter 22 (type reconstruction).
- Harper, R. (2016). *Practical Foundations of Programming Languages* (2nd ed.). Cambridge University Press. Chapter 47 (type-directed compilation). Free PDF at https://www.cs.cmu.edu/~rwh/pfpl/