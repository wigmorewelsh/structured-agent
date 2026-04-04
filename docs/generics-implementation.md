# Generics Implementation

Parametric polymorphism — generics — allows a single function definition to operate over values of any type, with the concrete type resolved at each call site. The SA language currently has no support for this: every function signature names concrete types, and the type checker rejects any call where the argument type does not match exactly. This document describes the implementation of generics in SA, building directly on the typed AST pipeline described in [typed-ast-pipeline.md](typed-ast-pipeline.md).

The theoretical basis is the Hindley-Milner algorithm (Damas and Milner, "Principal type-schemes for functional programs", *POPL 1982*, https://dl.acm.org/doi/10.1145/582153.582176), though SA does not require full type inference. Type variables appear only in explicitly annotated function signatures — the programmer writes `fn head<T>(list: List<T>): Option<T>` — and are resolved by one-pass left-to-right unification at each call site. Pierce's *Types and Programming Languages* (MIT Press 2002, ch. 22) and Harper's *Practical Foundations of Programming Languages* (Cambridge 2016, ch. 47) provide the formal grounding for the unification and substitution steps respectively.

## Prerequisites

All four phases of the typed AST pipeline are complete. The type checker produces a `TypedModule` in which every expression carries a `ty: ast::Type` and every call carries a `resolved: String` and `kind: FunctionKind`. The bytecode emitter reads from this structure directly and requires no side-channels. The consequence is that generics are, from the emitter's perspective, a non-event: the type checker resolves `T = String` at the call site, writes `Type::String` into the `TypedExpression` node, and the emitter never encounters a type variable.

## Syntax

Generic functions are declared with a type parameter list immediately after the function name, following the same angle-bracket convention used for `List<T>` and `Option<T>` in type positions:

```/dev/null/example.sa#L1-3
fn head<T>(list: List<T>): Option<T> {
    ...
}
```

Type parameters declared this way are scoped to the function body and its signature. A call site passes a concrete `List<String>` and the checker unifies `T = String`, yielding a return type of `Option<String>`.

The `extern fn` declaration follows the same syntax, allowing native functions to declare themselves as generic.

## Implementation Phases

### Phase G1: AST changes — done

Add `Generic(String)` to `ast::Type`. Add `type_params: Vec<String>` to `ast::Function` and `ast::SigFunction`. This phase is purely additive: no existing code paths change, and all existing tests continue to pass.

### Phase G2: Parser support — done

Extend `parse_type()` in [`compiler/parser.rs`](../src/structured-agent/src/compiler/parser.rs) to emit `Type::Generic(name)` for identifiers that are not `List`, `Option`, or the scalar type keywords. Currently those identifiers fall through to `Type::Struct(name)`; the type checker already distinguishes struct names from other names, so parsing them uniformly and letting the checker resolve them is safe. Add `<T, U, ...>` syntax after the function name in both `parse_function` and `parse_sig_function` (sig generic function support is deferred; the parser change is minimal and may as well be consistent). The `<` and `>` characters are already handled by `lex_char` for `List<T>` and `Option<T>`, so no lexer changes are required.

### Phase G3: Type checker — validation and unification — done

Three changes to [`typecheck/checker.rs`](../src/structured-agent/src/typecheck/checker.rs).

First, the internal `FunctionSignature` struct gains a `type_params: Vec<String>` field. When the checker builds a signature from an `ast::Function`, it copies the type params across.

Second, `validate_type` currently rejects any `Type::Generic` (or bare struct name that turns out to be a type variable) that is not in the struct registry. It must be extended to accept a `Generic(name)` where `name` is in the enclosing function's type parameter list. This requires passing the current function's type params down to `validate_type`.

Third, `check_call` currently uses raw `==` equality to compare each argument's resolved type against the corresponding formal parameter type. For a call to a generic function, it must instead build a substitution map by unifying each formal parameter type against the actual argument type left-to-right, then apply the substitution to the return type to produce the concrete return type written into the `TypedExpression`. The unification is bounded: type variables appear only in explicit signatures, so a single left-to-right pass without an occurs check is sufficient. Error messages name the concrete types after substitution, not the type variables.

### Phase G4: End-to-end tests — done

Integration tests covering: calling a generic function with a concrete type, calling with a mismatched type, calling a generic `extern fn`, and calling a function that is generic in multiple type parameters. Tests live alongside the existing tests in the `typecheck` and integration test modules.

### Phase G5: Final cleanup — not started

Address any clippy warnings introduced across the previous phases. Remove any dead code or intermediate scaffolding. Ensure all existing tests continue to pass.

## Future Work

The following are out of scope for this implementation and are deferred to future work.

Generic `sig` declarations — `sig fn map<T, U>(list: List<T>, f: ...): List<U>` — raise questions about how a sig's type parameters interact with the module parameter system and the `in M` constraint. These are deferred until the module system's Phase 6 (contract matching) is more settled. See [module-system-implementation.md](module-system-implementation.md).

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