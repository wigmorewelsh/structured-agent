# Type System Abstractions

SA has parametric polymorphism, ML-style signatures, and module parameters. These together cover the ground that most production languages reach only partially. The questions now are: which further abstractions are worth adding, what order matters, and what type checker architecture makes additions tractable without a rewrite each time.

## Where SA Stands

Parametric polymorphism, implemented as described in [generics-implementation.md](generics-implementation.md), allows functions to abstract over types. Signatures, described in [module-system.md](module-system.md), allow modules to abstract over implementations. Module parameters extend this into dependency injection with static verification. Together these cover three of the four classical forms of polymorphism identified by Cardelli and Wegner ("On Understanding Types, Data Abstraction, and Polymorphism", ACM Computing Surveys 1985, https://dl.acm.org/doi/10.1145/6041.6042): parametric, inclusion (via signatures acting as supertypes), and ad hoc via the module system. What is absent is a clean mechanism for constrained generics — type parameters bounded by required behaviour — which is the gap that prevented languages like early C# and Java from abstracting over numeric operations.

The actor system adds a fourth dimension: actors are modules with identity and state, and their interaction patterns raise questions about protocol safety, capability restriction, and effect tracking that the current type system cannot express. The theory bearing on these questions — session types, graded modal types, information flow types, and algebraic effects — is not independent. They are related projections of the same underlying structure, a point developed in [actor-system.md](actor-system.md) and expanded here.

## Constrained Generics

The most immediate gap. A type variable without constraints accepts any type, which means the function body can do nothing with the value except pass it around. To write `fn sort<T>(list: List<T>): List<T>` the checker needs to know that `T` supports comparison. The mechanism is a bound: `<T: Comparable>`, where `Comparable` is a named signature.

SA already has signatures. The extension required is to allow a signature name as a bound on a type parameter, with the checker verifying at each call site that the concrete type satisfies the bound. This is precisely the type class mechanism from Haskell (Wadler and Blott, "How to Make Ad-Hoc Polymorphism Less Ad Hoc", POPL 1989, https://dl.acm.org/doi/10.1145/75277.75283), expressed through SA's existing sig vocabulary rather than as a separate construct. The numeric operator problem — where .NET before C# 11 required duplicate implementations for each numeric type because `+` could not be abstracted — is resolved exactly here. A `Numeric` sig declaring the required operators, combined with a bounded type parameter, allows a single generic implementation.

## Higher-Kinded Types

Higher-kinded types (HKTs) allow abstraction over type constructors, not just types. `List` and `Option` both have kind `* -> *`: given a type, they produce a type. A function parameterised over such a constructor can express `map` once, for any container, rather than separately for each:

```/dev/null/example.sa#L1-3
sig Functor<F> {
    fn map<A, B>(fa: F<A>, f: Fn(A): B): F<B>
}
```

Here `F` is a type variable of kind `* -> *`, not kind `*`. The checker must track kinds alongside types.

The timing argument for HKTs is architectural. A type system that lacks them ends up with duplicated function families: `map_list`, `map_option`, `map_result`. These accumulate in the standard library and cannot be unified after the fact without breaking changes. The kind system is much easier to design in before the standard library grows than to retrofit. Rust's absence of HKTs is a known limitation that prevents a clean `Functor`/`Monad` abstraction in the standard library, and adding them now would require significant backwards-incompatible changes to trait resolution.

## Typestate for Actor References

The actor-system document identifies an open question: what type does `spawn<SomeActor>("id")` return? The answer proposed there is `ActorRef<SomeActor>`, but this raises the question of what the reference's type should express about the actor's current protocol state.

Typestate, introduced by Strom and Yemini ("Typestate: A Programming Language Concept for Enhancing Software Reliability", IEEE Transactions on Software Engineering 1986, https://ieeexplore.ieee.org/document/6312929), tracks discrete states as part of a value's type. Applied to actor references, the type becomes `ActorRef<Module, State>`, where `State` is a declared state index. Methods transition the state:

```/dev/null/example.sa#L1-4
fn accumulate(ref: ActorRef<Planner, Accumulating>): ActorRef<Planner, Accumulating>
fn query(ref: ActorRef<Planner, Accumulating>): ActorRef<Planner, Ready>
```

Calling `query` on an `ActorRef<Planner, Ready>` is a type error. This is a substantial subset of session types — statically checkable protocol ordering — at a fraction of the implementation cost. The equirecursive type checking required by full session types (Honda, Vasconcelos, Kubo, "Language Primitives and Type Discipline for Structured Communication-Based Programming", ESOP 1998) is not needed; a finite state index is sufficient for the actor patterns SA describes. Typestate is the bridge between the current untyped `ActorRef` and full session-typed channels, and can be layered on without redesigning the actor model.

## Row Polymorphism

Row polymorphism extends structural typing to open records. A function that accepts "any struct that has at least a `name: String` field" does not name a specific struct type — it accepts any record whose row contains that field:

```/dev/null/example.sa#L1-2
fn display<R: { name: String }>(r: R): String { r.name }
```

This is used in OCaml's object system and in Elm and PureScript for record operations. It composes naturally with generics and avoids the need to declare an interface for every combination of fields a utility function might need. The theoretical foundation is Rémy's row variables ("Type Inference for Records in a Natural Extension of ML", in Theoretical Aspects of Object-Oriented Programming, MIT Press 1994).

## Graded Modal Types

The actor-system document identifies that session types, taint tracking, and Contextual Modal Type Theory are "different projections of the same underlying structure" — types carrying an index that constrains usage, where the index is protocol state, trust level, or computational context respectively. The published formalisation of this observation is graded modal types.

In a graded type system, a modality is annotated with an element of a semiring (or preordered monoid). The grade simultaneously encodes resource usage, trust provenance, and capability. A value of type `◻_r A` can only be used in a context consistent with grade `r`. The checker propagates grades through type derivations and rejects programs where grade constraints are violated.

Granule (Orchard, Liepelt, Eades, "Quantitative Program Reasoning with Graded Modal Types", ICFP 2019, https://dl.acm.org/doi/10.1145/3341714) implements this. A single graded modality carries trust level, usage count (linearity), and capability in the same annotation. The three systems SA needs — taint tracking, session type state transitions, and CMTT context bounding — are instances of graded modalities with different semiring structures.

The practical question is what semiring captures `trust level × protocol state × context provenance` for SA. This is a design question that query-based or constraint-based checker architecture does not answer; it is a mathematical question about the algebraic structure of the combined index space. What the checker architecture does determine is whether that index can be propagated at expression granularity tractably — see the type checker design section below.

## Algebraic Effects

The `!` context injection operator, LLM calls, storage reads, and `spawn` are all effects: operations that do more than return a value. Currently they are invisible to the type system. A function that calls `!` looks identical in its signature to one that does not.

Algebraic effects (Plotkin and Power, "Notions of Computation Determine Monads", FoSSaCS 2002; Bauer and Pretnar, "Programming with Algebraic Effects and Handlers", JLAMP 2015, https://doi.org/10.1016/j.jlamp.2014.02.001) represent effects as named operations that can be declared, composed, and handled. A function signature carries its effect set:

```/dev/null/example.sa#L1-2
fn summarise(): String ! [LLM, Storage]
fn pure_fn(): String
```

The `in M` constraint already places SA close to effect typing: LLM-generated code is restricted to the operations declared in sig `M`. Making this explicit in the type of every function — not just generated code — turns the guardrail from a convention into a mechanically checked constraint. An actor handles the effects that occur within it, which maps cleanly onto the handler model: the actor's execution boundary is an effect handler scope.

## Capability Types and Information Flow

These are related but distinct mechanisms, and the distinction matters for SA.

Information flow typing, as described in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md), tracks where data came from. The trust lattice answers "can this data flow to this context?" A `String + Tainted` value carries a provenance label; crossing a trust boundary requires a declared boundary function. This is the Decentralised Label Model of Myers and Liskov ("A Decentralised Model for Information Flow Control", SOSP 1997, https://dl.acm.org/doi/10.1145/268998.266669), applied to SA's module system.

Object-capability security tracks what code is authorised to do. The question is "does this code hold the right to perform this action?" Authority is exactly what your references grant — no ambient authority, no other path to obtain permission. Miller's "Robust Composition" thesis (2006, http://www.erights.org/talks/thesis/markm-thesis.pdf) is the canonical treatment.

The two axes are orthogonal. A trusted actor with only a `Read` capability on a storage module is a coherent combination: high trust provenance, restricted authority. `String + Tainted` is a data label; `ActorRef<Store> with [Read]` is an authority token. SA's module system already implements capabilities implicitly — a module that holds only a `storage.Storage` sig parameter can call only `read` and `write`. The sig acts as a capability token. The gap is that this restriction does not transfer to the runtime value `ActorRef<T>`: passing an actor reference to another function loses the capability restriction that the sig would have enforced at the declaration site. Value-level capabilities would close this gap without replacing the information flow system.

## Type Checker Design

The three mechanisms that most affect checker architecture are HKTs (which introduce kind checking as a separate concern from type checking), graded types (which require grade propagation at expression granularity), and constrained generics (which require signature satisfaction checks at call sites). A traditional sequential pass-based checker handles each of these by adding a new pass, which compounds: kind checking before type checking before taint checking before session type checking is a fragile dependency chain.

Three design alternatives address this.

### Bidirectional Type Checking

Bidirectional checking alternates between two modes: check mode, where a term is verified against a known expected type, and synthesise mode, where a type is derived from a term's structure. Annotated function signatures feed into check mode, which propagates expected types top-down. Sub-expressions synthesise upward. For HKTs, check mode propagates kind expectations from a signature down into the body, eliminating the need for a separate kind-inference pre-pass.

Dunfield and Krishnaswami's "Complete and Easy Bidirectional Type Checking for Higher-Rank Polymorphism" (ICFP 2013, https://dl.acm.org/doi/10.1145/2544174.2500582) gives a practical algorithm. Their 2021 survey "Bidirectional Typing" (ACM Computing Surveys, https://dl.acm.org/doi/10.1145/3450952) covers extensions to richer type systems including graded types and dependent types. Since SA already requires explicit annotations on generic functions, the full inference burden is lighter than Haskell's, and bidirectional checking fits the existing annotation model directly.

### Constraint-Based Checking (HM(X))

The HM(X) framework separates constraint generation from constraint solving. The AST walk emits a set of constraints — type equalities, kind constraints, grade inequalities, sig obligations — without solving them. A separate solver discharges the constraints. The `X` is pluggable: swapping in a different solver adds a new type system feature without changing the constraint generation phase.

GHC's OutsideIn(X) algorithm (Vytiniotis, Jones, Schrijvers, Sulzmann, Journal of Functional Programming 2011, https://www.microsoft.com/en-us/research/publication/outsideinx-modular-type-inference-with-local-assumptions/) uses this approach for type classes, GADTs, and type family constraints within a single framework. For SA, taint constraints, kind constraints, sig satisfaction obligations, and grade inequalities would all be constraint forms emitted by the same generation phase and solved together. This is the mechanism that makes the "unified checker" observation from the actor-system document tractable in practice: not a single semiring designed up front, but a single constraint framework with pluggable solvers for each subsystem.

### Query-Based Architecture

A query-based checker (as used in rust-analyzer via Salsa, https://github.com/salsa-rs/salsa, and in Roslyn) computes type information lazily on demand, caches results, and invalidates only what has changed. "What is the type of this expression?" and "what kind does this type constructor have?" are queries that recurse through dependencies and return cached answers on subsequent calls.

For HKTs, this directly addresses the pass-ordering problem. In a sequential checker, kind information must be available before type information for the same expression. In a query-based checker, the kind query and the type query are independent; the kind query is issued from within the type query when needed. Mutual recursion between queries (which can arise with mutually recursive type constructors) is handled by the query system's cycle detection rather than by explicit cycle-breaking logic in the checker.

For graded types, query-based architecture makes value-level grade propagation tractable. Propagating grades through every sub-expression eagerly in a pass-based checker is expensive. Computing the grade of an expression on demand, caching it, and recomputing only when a dependency changes is the query model's natural mode of operation.

### Composition

The three designs are not alternatives — they compose. Bidirectional checking provides the top-level algorithm (check or synthesise at each node). Constraint generation and solving separates the SA-specific extensions (grades, taint, kind constraints) from the core algorithm. Query-based infrastructure handles incremental recomputation and the ordering problem. The current checker performs HM unification at call sites; the path forward is to route that unification through a constraint solver, layer bidirectional checking around it, and host the whole thing in a query framework.

```
/dev/null/checker-architecture.md#L1-1
Source AST
   │
   ▼
Bidirectional elaboration  ──────────────────────┐
(check / synthesise modes)                        │
   │                                              │
   ▼                                              ▼
Constraint emission                        Query cache
(types, kinds, grades, sig obligations)    (incremental, on-demand)
   │
   ▼
Constraint solver (HM(X))
  ├── Unification (types, kinds)
  ├── Grade solver (semiring inequalities)
  └── Sig satisfaction checker
   │
   ▼
Elaborated typed AST
```

## Priority and Sequencing

Constrained generics build directly on existing signatures and are the most pressing gap — without them, any function that requires behaviour from its type parameter is blocked. HKTs are an architectural decision with a closing window: the standard library and any sig involving container types will progressively bake in assumptions that are hard to undo. Both should precede a growing standard library.

Typestate for `ActorRef<T>` resolves an explicitly open question in the actor-system design and is achievable with a finite state index rather than full session types. It is the right next actor-related addition. Graded modal types and algebraic effects are longer-horizon work; the constraint-based checker architecture is the foundation that makes them additive rather than disruptive when the time comes.

## See Also

- [generics-implementation.md](generics-implementation.md) — current generics implementation
- [module-system.md](module-system.md) — signatures and module parameters
- [actor-system.md](actor-system.md) — actor design and session type relationship
- [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — taint tracking and trust lattice
- Cardelli, L. and Wegner, P. (1985). "On Understanding Types, Data Abstraction, and Polymorphism." ACM Computing Surveys. https://dl.acm.org/doi/10.1145/6041.6042
- Wadler, P. and Blott, S. (1989). "How to Make Ad-Hoc Polymorphism Less Ad Hoc." POPL 1989. https://dl.acm.org/doi/10.1145/75277.75283
- Strom, R. and Yemini, S. (1986). "Typestate: A Programming Language Concept for Enhancing Software Reliability." IEEE Transactions on Software Engineering. https://ieeexplore.ieee.org/document/6312929
- Rémy, D. (1994). "Type Inference for Records in a Natural Extension of ML." In Theoretical Aspects of Object-Oriented Programming. MIT Press.
- Orchard, D., Liepelt, V., Eades, H. (2019). "Quantitative Program Reasoning with Graded Modal Types." ICFP 2019. https://dl.acm.org/doi/10.1145/3341714
- Bauer, A. and Pretnar, M. (2015). "Programming with Algebraic Effects and Handlers." JLAMP. https://doi.org/10.1016/j.jlamp.2014.02.001
- Myers, A. and Liskov, B. (1997). "A Decentralised Model for Information Flow Control." SOSP 1997. https://dl.acm.org/doi/10.1145/268998.266669
- Miller, M. (2006). "Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control." PhD thesis. http://www.erights.org/talks/thesis/markm-thesis.pdf
- Dunfield, J. and Krishnaswami, N. (2013). "Complete and Easy Bidirectional Type Checking for Higher-Rank Polymorphism." ICFP 2013. https://dl.acm.org/doi/10.1145/2544174.2500582
- Dunfield, J. and Krishnaswami, N. (2021). "Bidirectional Typing." ACM Computing Surveys. https://dl.acm.org/doi/10.1145/3450952
- Vytiniotis, D., Jones, S. P., Schrijvers, T., Sulzmann, M. (2011). "OutsideIn(X): Modular Type Inference with Local Assumptions." Journal of Functional Programming. https://www.microsoft.com/en-us/research/publication/outsideinx-modular-type-inference-with-local-assumptions/
- Honda, K., Vasconcelos, V., Kubo, M. (1998). "Language Primitives and Type Discipline for Structured Communication-Based Programming." ESOP 1998.
- Rossberg, A. (2015). "1ML — Core and Modules United." ICFP 2015. https://people.mpi-sws.org/~rossberg/1ml/