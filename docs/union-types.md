# Union Types

Union types let SA express "this parameter accepts `Image` or `Audio`" at the type level. Without them, functions that must handle more than one media kind either carry duplicate signatures or fall back to an untyped representation — both of which push complexity onto the programmer and hide errors from the type checker. With union types, the constraint is stated once in the type and the checker enforces it everywhere.

## Motivation

SA's prelude now includes `Image`, `Audio`, and `Link` (see [native-content-types.md](native-content-types.md)). These types exist precisely because different media require different handling by LLM APIs and agent protocols, and naming them at the type level is what allows the compiler to verify that distinction. But the three types are not independent in practice: a function that processes any media input, or a struct field that holds an attachment of any kind, must accept all of them together. Union types are the mechanism for expressing that grouping without discarding the distinctions the individual types carry.

## Syntax

An inline union is written as pipe-separated types in any type position:

```/dev/null/example.sa#L1-3
fn describe(media: Image | Audio): String { ... }

struct Message {
    attachment: Image | Audio,
}

fn process(items: List<Image | Audio>): List<String> { ... }
```

Named aliases use a `type` declaration:

```/dev/null/alias.sa#L1-2
type Media = Image | Audio
type Content = String | Image | Audio | Link
```

A named alias is transparent. `Image` is directly assignable to `Media` without a cast or conversion. The alias expands to its constituent variants during type checking and has no runtime cost — it is a name for a set of types, not a wrapper around a value.

## Usage in All Type Positions

Union types are valid wherever any other type is written: function parameter types, return types, struct field types, and as type arguments to parameterised types. `fn get(): Image | Audio` is as valid as `fn process(x: Image | Audio): String`. There is no position in which a union is privileged or forbidden. `List<Image | Audio>` is a list that may contain both images and audio values, not a list restricted to one kind. This generality is necessary: restricting unions to certain positions would create a smaller feature with a longer list of exceptions.

## Narrowing via Exhaustive Match

Once a value has a union type, the only way to operate on a concrete variant is through a `match` expression. SA does not permit `is`-style isinstance checks. A match arm narrows the type: within `Image(img) => ...`, the binding `img` has type `Image`, not `Image | Audio`. The type checker enforces exhaustiveness — omitting a variant is a compile error:

```/dev/null/match.sa#L1-6
fn describe(m: Media): String {
    match m {
        Image(img)   => caption(img),
        Audio(audio) => transcribe(audio),
    }
}
```

`match` is both an expression and a statement. As an expression, all arms must have the same return type; as a statement, the arms may diverge, but exhaustiveness is still required. The exhaustiveness requirement is the point: it forces the programmer to handle every variant the type admits, which is precisely the guarantee union types exist to provide. Isinstance checks allow a silent default to substitute for deliberate handling, which defeats the purpose.

Duplicate arms and arms naming types that are not members of the union are both compile errors.

## The Assignability Relation

The current type checker uses equality: `got == expected`. A concrete type is accepted if it matches the declared type exactly. Union types require a richer relation: `T` is assignable to `T | U` even though they are not equal. This is a partial order — subtyping — and it changes the fundamental checking operation.

The `is_assignable(from, to)` predicate replaces bare equality at every check site. For nominal, flat unions, the predicate is set containment after alias expansion: `from ≤ to` if and only if every variant in `from` appears in `to`. `Image ≤ Image | Audio` because `{Image} ⊆ {Image, Audio}`. `Image | Audio ≤ Image` fails because `Audio` is not in `{Image}`.

This is not a local addition to the checker. The checking relation is the core of the type checker; replacing equality with a partial order affects every site where the checker decides whether an expression's type satisfies its context. Cardelli and Wegner's treatment of inclusion polymorphism (ACM Computing Surveys, 1985, https://dl.acm.org/doi/10.1145/6041.6042) gives the foundational account of subtyping as a partial order and its interaction with function types. For function types the rule is contravariant in parameters and covariant in return types: `(Image | Audio) -> String ≤ Image -> String` because accepting more is compatible with accepting less, but `Image -> String` is not assignable to `(Image | Audio) -> String` because the callee may pass an `Audio` the function cannot handle.

SA's unions at this stage are structural only — the subtype relation is set containment, not a class hierarchy — so no variance in generic parameters is required yet. That question arises when higher-kinded types arrive.

## Alias Normalization and Overlapping Unions

Aliases that include other aliases produce overlapping union sets:

```/dev/null/overlap.sa#L1-3
type Media = Image | Audio
type AV    = Audio | Link
type Mixed = Media | AV
```

`Mixed` expands to `Image | Audio | Audio | Link`, which after deduplication normalises to `Image | Audio | Link`. All unions must be reduced to flat sets of nominal types at resolve time. The normalization step expands every named alias recursively and removes duplicates. Both assignability and exhaustiveness checking operate on the normalized form. Keeping un-normalized unions in the type checker would require ad hoc handling of every alias shape encountered during checking; normalization at resolve time makes the checker uniform.

The interaction between overlapping aliases and the type hierarchy is illustrated in the diagram below.

```/dev/null/diagram.txt#L1-20
Type positions
──────────────────────────────────────────────────────────────────

  fn describe(m: Media): String
               │
               │  alias expansion (resolve time)
               ▼
         Union { Image, Audio }     (normalized flat set)
               │
               │  match scrutinee
              ╱ ╲
     Image(img)   Audio(audio)
         │               │
    type: Image     type: Audio    (narrowed in arm body)


Named aliases
──────────────────────────────────────────────────────────────────

  Media  ──expand──►  { Image, Audio }
  AV     ──expand──►  { Audio, Link  }
                         ↓ union + deduplicate
  Mixed  ──expand──►  { Image, Audio, Link }
```

## Type System Impact

### AST

`ast::Type` in `structured-agent/src/structured-agent-ast/src/ast/mod.rs` is currently a plain struct with a `path` and a `args` field. It represents only named types with optional type arguments. It cannot represent a union. It must become an enum:

```/dev/null/ast_type.rs#L1-4
pub enum AstType {
    Named { path: AstPath, args: Vec<AstType> },
    Union(Vec<AstType>),
}
```

Every pattern match on `ast::Type` throughout the parser, type checker, and display implementations needs updating. This is broad but mechanical: each match gains a `Union` arm, and sites that assumed a single path must be updated to handle multiple variants.

### Runtime

The runtime `Type` enum in `structured-agent/src/structured-agent-runtime/src/types.rs` gains a corresponding variant:

```/dev/null/runtime_type.rs#L1-5
pub enum Type {
    Named(DefinitionPath),
    Parameterized(DefinitionPath, Vec<Type>),
    Generic(String),
    Union(Vec<Type>),    // new
}
```

### Arrow

Arrow's `UnionArray` infrastructure is already exercised by `OptionValue` in `structured-agent/src/structured-agent-runtime/src/runtime_value/option.rs`. `Union(variants)` maps to `DataType::Union(fields, UnionMode::Dense)`. The dense union mode is appropriate: SA unions are closed and nominal, and every value carries a type tag identifying its variant. Sparse union mode, which stores each variant in a separate buffer, would waste memory for the common case of small variant sets.

### New AST Nodes

`TypeAlias` is a new `Definition` variant:

```/dev/null/ast_nodes.rs#L1-2
TypeAlias { name: String, ty: AstType, span: Span }
```

`Match` is a new expression and statement form. Each arm carries the variant name, a binding name, a `BindingId` assigned during elaboration, and the arm body:

```/dev/null/ast_nodes.rs#L4-11
pub struct MatchArm {
    pub variant_name: String,
    pub binding:      String,
    pub binding_id:   BindingId,
    pub body:         Expression,
    pub span:         Span,
}
```

## Exhaustiveness Checking

When checking a `match` expression, the type checker resolves the scrutinee type to its normalized union variants, expanding any aliases. It then verifies that the set of variant names across all arms is exactly equal to the set of variants in the union — no more, no fewer. Missing a variant is an "incomplete match" error citing the absent type. Naming a type not in the union is an "unreachable arm" error. Repeating a variant is a "duplicate arm" error.

Exhaustiveness in this setting is decidable and cheap. SA's unions are closed — there is no open-world extension — and finite. The check is a set equality test on nominal names. The more complex exhaustiveness algorithms required for algebraic types with constructor patterns (the matrix algorithm of Maranget, "Warnings for Pattern Matching", JFP 2007, https://www.cambridge.org/core/journals/journal-of-functional-programming/article/warnings-for-pattern-matching/5B9B7DD64F3F9DCD2A0F97EB7B1D56A0) are not needed here; the union structure is flat and every variant is a single name.

## Undecidability and Future Concerns

For the immediate design — nominal, closed, finite, non-recursive unions — subtype checking is decidable. It is set containment on a finite set. No concerns arise here.

Three planned features interact with union types in ways that reintroduce non-trivial theoretical complexity.

The first is recursive type aliases. `type Expr = Int | List<Expr>` is structurally well-formed but its alias expansion does not terminate without an occurs-check. This is the same problem as the occurs-check in Hindley-Milner unification, applied to alias expansion rather than type variable unification. The fix is the same in structure: before expanding an alias, record it in a seen-set and error if it recurs. Without this guard, the normalizer loops on any recursive alias.

The second concern arises from higher-kinded types, which are planned (see [type-system-abstractions.md](type-system-abstractions.md)). Once type constructors can be abstracted, the question of whether `F<A | B>` equals `F<A> | F<B>` — distributivity of a type constructor over a union — becomes live. For `List` this equation feels natural, but it requires that `List` be declared covariant in its parameter. Unrestricted distributivity with higher-kinded constructors reaches the territory of System F-omega (Girard, "Interprétation Fonctionnelle et Élimination des Coupures de l'Arithmétique d'Ordre Supérieur", PhD thesis, Université Paris VII, 1972), where type equality is undecidable in general. The safe path is to restrict distributivity to explicitly covariant constructors and require an annotation at the site of each claim.

The third concern is the introduction of intersection types. If an `&` operator is ever added to SA's type grammar, the combined union-and-intersection subtype relation is PSPACE-complete in the general case and has known undecidable and exponential-time fragments. TypeScript has encountered both undecidable and practically diverging type-checking cases with complex union and intersection combinations (GitHub issues #14833, #34901 among others). The lesson from TypeScript is that once both operators are in the language the subtype relation requires explicit complexity budget and escape hatches, not just a cleaner algorithm.

None of these concerns apply to the current design. They are relevant to decisions made about later features, and the current design should not be made more complicated to pre-empt them.

## The Option<T> Tension

`Option<T>` and `T | Unit` are isomorphic as values: both represent either a value of type `T` or nothing. `OptionValue` in `structured-agent/src/structured-agent-runtime/src/runtime_value/option.rs` has specific Arrow semantics built around `none` and `some` cases. With union types in the language, programmers will expect `String | Unit` to behave identically to `Option<String>`, and the expectation is reasonable — they express the same thing.

There are two coherent resolutions. `Option<T>` becomes syntactic sugar for `T | Unit`, and the general union machinery handles both. The runtime's `OptionValue` becomes an instance of the general union representation, and `none`/`some` become the standard `Unit` and `T` match arms. The alternative is to keep `Option<T>` as a separate type with its own runtime representation and accept that two isomorphic types exist with different spellings. The second option is incoherent to users: they will write `String | Unit` and be surprised when it differs in behaviour or representation from `Option<String>`.

This is a design decision that must be made before both implementations exist. Unifying them after the fact requires migrating `OptionValue`'s Arrow encoding, updating all existing code that pattern-matches on `Option`, and ensuring the unified representation preserves the dense union mode that the current `OptionValue` relies on for correctness.

## Dependency on the Constraint Architecture

The assignability relation cannot be correctly wired into the type checker until the `Unifier` moves from `elaboration.rs` into the constraint solver described in [typecheck-constraint-architecture.md](typecheck-constraint-architecture.md). The reason is that union subtype constraints — `T ≤ Image | Audio` — need to be solved in the same pass as type equality constraints. If the `Unifier` remains in elaboration and subtyping is added as a separate pass, the two interact in ways that require backtracking: a type variable unified in elaboration may later turn out to violate a union subtype constraint that has not yet been checked, and by then the unification decision is committed. Running both inside a single constraint solver avoids the commitment problem: constraints are emitted by elaboration and solved together, so subtype and equality constraints see each other's consequences simultaneously.

This is not a local implementation detail. Union subtype constraints are a qualitatively different form of constraint from the equality constraints HM unification handles, and adding them to an elaboration-time unifier rather than a constraint solver produces a checker that is correct on simple cases but wrong on cases where equality and subtyping interact — exactly the cases that arise in generic functions with union-typed parameters.

## References

- Cardelli, L. and Wegner, P. (1985). "On Understanding Types, Data Abstraction, and Polymorphism." ACM Computing Surveys. https://dl.acm.org/doi/10.1145/6041.6042
- Maranget, L. (2007). "Warnings for Pattern Matching." Journal of Functional Programming. https://www.cambridge.org/core/journals/journal-of-functional-programming/article/warnings-for-pattern-matching/5B9B7DD64F3F9DCD2A0F97EB7B1D56A0
- TypeScript union types: https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#union-types
- Arrow union layout: https://arrow.apache.org/docs/format/Columnar.html#union-layout
- `structured-agent/src/structured-agent-runtime/src/runtime_value/option.rs` — existing `UnionArray` usage
- `structured-agent/src/structured-agent-ast/src/ast/mod.rs` — `ast::Type` struct requiring extension
- `structured-agent/src/structured-agent-runtime/src/types.rs` — runtime `Type` enum
- [native-content-types.md](native-content-types.md) — `Image`, `Audio`, and `Link`
- [type-system-abstractions.md](type-system-abstractions.md) — HKT plans and constraint checker architecture
- [typecheck-constraint-architecture.md](typecheck-constraint-architecture.md) — `Unifier` migration to solver
- [constrained-generics.md](constrained-generics.md) — interaction with trait bounds on union-typed parameters
