# Constrained Generics

SA's parametric polymorphism, described in [generics-implementation.md](generics-implementation.md), allows functions to abstract over types. A type variable without constraints accepts any type, which means the function body can do nothing with the value except pass it around. To write a function that requires its type parameter to support some operation — comparison, arithmetic, serialisation — the language needs a mechanism to express and enforce that requirement. Constrained generics provide it.

The mechanism has two parts: traits, which name the required behaviour, and bounds, which attach trait requirements to type parameters. Together they close the gap that prevented languages like early Java and C# from abstracting over numeric operations — a `Numeric` trait declaring the required operators, combined with a bounded type parameter, allows a single generic implementation where those languages required one copy per numeric type.

## Traits

A trait declares a named set of functions that a type must provide. Traits in SA are distinct from sigs: sigs describe module interfaces, traits describe type behaviour. The distinction matters because sigs are satisfied by modules at wiring time, while traits are satisfied by structs at definition time. The two may eventually converge — 1ML (Rossberg, "1ML — Core and Modules United", ICFP 2015, https://people.mpi-sws.org/~rossberg/1ml/) demonstrates that modules and types are the same abstraction at different scales — but SA treats them separately for now.

A trait declaration names the functions that satisfying types must implement. The special type `Self` refers to the implementing type:

```/dev/null/trait.sa#L1-3
trait Add {
    fn add(self: Self, other: Self): Self
}
```

The four arithmetic traits built into SA are `Add`, `Sub`, `Mul`, and `Div`. `Int` satisfies all four by default, registered as built-in implementations without requiring SA source code. User-defined structs may implement any trait by providing an `impl` block.

## Bounds

A type parameter is bounded by appending a colon and one or more trait names after the parameter name. Multiple bounds use `+` as a separator, following the same `T + L` qualification syntax used for taint labels in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md):

```/dev/null/bounds.sa#L1-2
fn sum<T: Add>(items: List<T>): T
fn clamp<T: Add + Sub>(v: T, lo: T, hi: T): T
```

At each call site, the type checker unifies the type parameter against the concrete argument type, then verifies that the concrete type satisfies every bound. Calling `sum` with a `List<Int>` succeeds because `Int` implements `Add`. Calling it with a `List<String>` is a type error because `String` does not.

## Implementing Traits

An `impl` block declares that a named type satisfies a named trait and provides the required functions:

```/dev/null/impl.sa#L1-7
struct Vec2 {
    x: Int,
    y: Int,
}

impl Vec2: Add {
    fn add(self: Vec2, other: Vec2): Vec2 {
        return self
    }
}
```

The type checker validates the impl at the point of declaration: every function named in the trait must appear in the impl block with a compatible signature. A missing function is a compile-time error. An impl that names an unknown trait is a compile-time error.

For the current pass, impl blocks must live in the same module as the type they implement. This restriction is discussed further in the coherence section below.

## The Type Checker

The type checker stores trait definitions and impl registrations as two maps on the `TypeChecker` struct. Trait definitions map trait names to their function declarations. Impl registrations map type names to the set of traits they satisfy.

When a function with bounded type parameters is called, the checker builds the substitution map by unifying formal parameter types against actual argument types, as described in [generics-implementation.md](generics-implementation.md). After unification, for each bounded type parameter, the checker resolves the concrete type from the substitution map and verifies it appears in the impl registry for every declared bound:

```
call site: sum(list_of_int)
  → unify T = Int
  → check: Int ∈ impls[Add]  ✓
```

Three new error types cover the failure cases: `TraitBoundNotSatisfied` when the concrete type lacks an impl for a required trait, `UnknownTrait` when an impl names a trait that has not been declared, and `TraitImplMissingFunction` when an impl block omits a function required by the trait.

## Dispatch: Modular Implicits

Bounds are currently call-site constraints only. Making them executable — so that `add(x, y)` inside `fn sum<T: Add>` resolves to `T`'s concrete `add` — requires a dispatch strategy. SA's chosen direction is modular implicits (White, Bour, Yallop, "Modular Implicits", ML Workshop 2014, https://arxiv.org/abs/1512.01438): trait bounds become implicit module arguments, resolved by the compiler at each call site from the modules in scope. `fn sum<T: Add>(items: List<T>): T` compiles as though `Add_impl: AddModule` were an additional parameter, where `AddModule` is a module satisfying the `Add` sig. The compiler finds the right module from the call site's scope and passes it implicitly; the programmer writes nothing extra at the call site.

Each `impl` block produces a named module derived mechanically from the type and trait names, using SA's `::` module path separator: `impl Vec2: Add` creates the module `Vec2::Add` in scope within the defining module. Implicit resolution searches for a module at that path when a call site requires `Add` for `Vec2`. Programmers never write `Vec2::Add` directly in normal code — it exists as a resolved name for the implicit passing mechanism and for explicit disambiguation through parameterised module wiring when conflicts arise.

This strategy aligns with every dimension of SA's design. The implicit module argument is a constraint in HM(X) terms — the solver finds the right module per call site — bidirectional checking propagates it top-down, and the query cache stores resolved implicits per call site. For durable execution, the approach is the strongest available: generic functions retain one stable identity in serialised state, and because SA already needs hot-swap semantics for modules — updating an LLM or storage module under a running agent — the module versioning infrastructure applies to trait implementations without additional work. A code update to `Int`'s `Add` module does not disturb any paused execution inside a generic function; only future calls pick up the new module. The reasons other strategies were set aside, and a full comparison across runtime performance, code size, checker fit, and durable execution compatibility, appear in Appendix B.

### Why SA Avoids OCaml's Difficulties

OCaml's modular implicits proposal has been stalled since 2015. The difficulties are real but stem from specific properties of OCaml that do not apply to SA in its current form.

The deepest problem is coherence. OCaml's module system allows multiple modules satisfying the same sig to be in scope simultaneously — that is the point of the module system — and implicit resolution rules that are not "there can only be one" produce programmes whose behaviour changes depending on which modules are imported. SA's same-module restriction addresses this directly for concrete types: `impl Vec2: Add` must live in the module that defines `struct Vec2`, so there is at most one impl per type per trait by construction, and duplicates are a compile-time error at the definition site. No global search is required and no resolution ambiguity is possible for the cases the restriction covers.

The second problem is principal types. Hindley-Milner guarantees that every typeable expression has a unique most-general type; implicit resolution can break that guarantee by making a call site typeable under multiple distinct implicit arguments. SA requires explicit annotations on all generic functions — `fn sum<T: Add>(items: List<T>): T` declares the bound in the signature — which means implicit resolution at a call site is always solving for a known target rather than performing open-ended search. The principal types problem only bites when the compiler must simultaneously infer the type and the implicit argument; SA's annotation requirement prevents that case.

The third problem is ecosystem friction. OCaml's difficulties are substantially retrofit difficulties: adding implicits to a language with decades of existing code means every design decision must be compatible with patterns built around explicit functors (`Map.Make`, `Set.Make`) that nobody anticipated needing to replace. SA has no existing ecosystem to protect. The module system, sig system, and trait system are being shaped together from the start.

### What Remains Deferred

The same-module restriction that makes coherence tractable now becomes limiting once blanket impls arrive with higher-kinded types. `impl<T: Add> List<T>: Summable` belongs to no single module. Third-party impls — adding a trait to a type defined elsewhere — face the orphan problem. And two independently authored modules providing conflicting impls for the same type and trait create ambiguity the compiler cannot resolve by inspection alone. These cases are addressed by the coherence machinery described in Appendix A: glue modules with explicit imports for third-party impls, scope-based priority via the module import graph for most ordering questions, and explicit parameterised module wiring for genuine conflicts. Deliberate overlap between a blanket impl and a specific override uses a `specializes` annotation so the compiler can verify the relationship for the stated pair.

Generic structs (`struct Pair<A, B>`) are also deferred, as they require changes to struct definition, instantiation, and field access orthogonal to function generics.

## Coherence

Coherence is the guarantee that for any concrete type and any trait, at most one impl is visible at any call site. Without it, two parts of the same program can observe different behaviour for the same type under the same trait, making programs hard to reason about.

The same-module restriction enforces coherence for concrete types trivially: because `impl Vec2: Add` must live in the module that defines `struct Vec2`, there is exactly one place it can appear and duplicate impls are a compile-time error at the definition site. No global search is required.

The restriction does not scale to blanket impls. Once SA gains higher-kinded types, writing `impl<T: Add> List<T>: Summable` requires a home module for the impl that is neither the module defining `List` nor the module defining the concrete `T`. Determining whether two blanket impls overlap requires asking whether two type predicates have a common instance, which is undecidable in general (Harper and Morrisett, "Compiling Polymorphism using Intensional Type Analysis", POPL 1995, https://dl.acm.org/doi/10.1145/199448.199475).

SA's planned resolution, informed by the module parameterisation system already in place, is discussed in the coherence appendix below.

## See Also

- [generics-implementation.md](generics-implementation.md) — parametric polymorphism without bounds
- [module-system.md](module-system.md) — sigs, module parameters, and wiring
- [durable-execution.md](durable-execution.md) — execution state persistence and hot code update model
- [type-system-abstractions.md](type-system-abstractions.md) — the broader type system roadmap
- [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — taint labels and the `T + L` syntax
- Wadler, P. and Blott, S. (1989). "How to Make Ad-Hoc Polymorphism Less Ad Hoc." POPL 1989. https://dl.acm.org/doi/10.1145/75277.75283
- Cardelli, L. and Wegner, P. (1985). "On Understanding Types, Data Abstraction, and Polymorphism." ACM Computing Surveys. https://dl.acm.org/doi/10.1145/6041.6042
- Rossberg, A. (2015). "1ML — Core and Modules United." ICFP 2015. https://people.mpi-sws.org/~rossberg/1ml/
- White, L., Bour, F., Yallop, J. (2015). "Modular Implicits." ML Workshop 2014. https://arxiv.org/abs/1512.01438

---

## Appendix A: Coherence at Scale

The same-module restriction solves coherence for concrete types but becomes limiting when blanket impls arrive with higher-kinded types. The problem has three layers.

**The orphan problem.** If `List` is in the standard library and `Printable` is in a third-party library, `impl List<T>: Printable` belongs to neither module. Rust's solution is the orphan rule: an impl must be in the crate that defines either the type or the trait. This prevents third parties from adding behaviour to types they do not own, which is sometimes the right constraint and sometimes an obstacle.

SA's module system offers a cleaner alternative. A third-party glue module may provide an impl for types it imports, as long as it explicitly imports both the type and the trait. The explicit `use` declarations are the declaration of intent, and the impl is scoped to modules that import the glue module. This is similar to Swift's retroactive conformances (annotated `@retroactive`) but grounded in SA's explicit import model rather than an ad-hoc annotation.

**The conflict problem.** Two independently authored glue modules, both imported, both providing `impl List<T>: Printable`, create a genuine conflict that the compiler cannot resolve by priority inspection alone. This is the case where SA's parameterised module system applies directly. Rather than two conflicting impls floating in ambient scope, the entry module wires its choice explicitly:

```/dev/null/wiring.sa#L1-3
mod printing: printable::Printable = glue_a

use list::List
```

The module parameter slot `mod printing: printable::Printable` forces a single declaration of intent at the wiring site, following exactly the same pattern as wiring a storage or formatter dependency. The conflict becomes a missing wiring declaration rather than a silent coherence violation.

**The overlap problem.** With blanket impls, two impls may both apply to the same concrete type:

```/dev/null/overlap.sa#L1-2
impl<T: Debug> T: Display { ... }
impl Int: Display { ... }
```

Both apply to `Int`. Determining programmatically which is more specific requires solving overlap, which is undecidable in the general case. Three approaches exist in the literature, in ascending order of ergonomic cost.

*Explicit priority via `specializes`.* The specific impl declares which general impl it overrides:

```/dev/null/specializes.sa#L1-4
impl<T: Debug> T: Display {
    fn display(self: T): String { debug_format(self) }
}
impl Int: Display specializes <T: Debug> T: Display {
    fn display(self: Int): String { int_to_string(self) }
}
```

The compiler verifies the stated relationship for the declared pair — is `Int` a valid substitution for `T: Debug`? — which is decidable even though general overlap is not. Accidental overlap without a `specializes` annotation remains a hard error. This approach is similar to Rust's specialisation RFC (RFC 1210, https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md), which has remained unstable since 2015 due to soundness difficulties with associated types. The `specializes` form avoids those difficulties by making the override relationship explicit and unidirectional rather than inferred from specificity.

*Scope-based priority.* SA's module import graph provides a natural priority lattice analogous to Scala 3's use of class inheritance depth for `given` instance priority. A re-exporting module sits above the modules it imports in the graph; its impls take priority over theirs. Priority is encoded in module structure rather than annotations. The difficulty — two unrelated modules with no hierarchical relationship — resolves through the parameterised module mechanism described above.

*Instance chains.* Morris and Jones ("Instance Chains: Type Class Programming Without Overlapping Instances", ICFP 2010, https://dl.acm.org/doi/10.1145/1863543.1863596) replace overlapping instances with an explicit ordered sequence of cases, analogous to pattern matching on types:

```/dev/null/chains.sa#L1-5
impl chain T: Display {
    | T ~ Int    -> fn display(self: Int): String { int_to_string(self) }
    | T ~ String -> fn display(self: String): String { self }
    | Debug T    -> fn display(self: T): String { debug_format(self) }
}
```

Cases are tried in declaration order; the first match wins. No overlap detection is required. Coherence is guaranteed by construction. The cost is centralisation: all cases for a given trait-type combination must appear in one chain. Open chains — allowing external modules to prepend new cases — restore extensibility provided new cases are provably disjoint from existing ones, which is tractable for a single prepended case against a known existing chain.

SA's near-term path is `specializes` for deliberate single overrides and module parameterisation for genuine ambiguity, with instance chains held in reserve for cases where the combination of blanket impls and higher-kinded types makes the overlap structure too complex to express through pairwise annotations.

---

## Appendix B: Dispatch Strategy

Bounds are currently compile-time constraints only. Making them executable — so that `add(x, y)` inside `fn sum<T: Add>` resolves to `T`'s `add` — requires a dispatch strategy. The choice affects runtime performance, code size, and, critically for SA, compatibility with durable execution and hot code updates as described in [durable-execution.md](durable-execution.md). The type checker architecture SA is moving towards — bidirectional checking, HM(X) constraint solving, and query-based incremental computation, described in [type-system-abstractions.md](type-system-abstractions.md) — adds a further dimension to the comparison.

**Monomorphisation** is the approach taken by Rust and by `rustc_codegen_cranelift`, making it the most natural fit for SA's planned Cranelift backend. The compiler generates a separate concrete function for every distinct set of type arguments encountered at call sites: `fn double<T: Add>(x: T)` called with `Int` and `Vec2` produces `double_Int` and `double_Vec2`, each compiled independently against its concrete types. Runtime overhead is zero and the mental model is straightforward — every instantiation is an ordinary function. Against SA's future checker, the approach is workable: the HM(X) substitution map already provides the type arguments needed to drive specialisation, and the query cache can memoize specialised function bodies. The codegen concern sits outside the type checker proper, so the two phases remain decoupled. The durable execution story is poor. A paused execution serialises as being inside `double_Int`, a function whose identity is tied to a specific call site. A code update to `double` produces a new `double_Int`, requiring every serialised specialisation to be versioned independently. The number of versioned artefacts scales with the cartesian product of generic functions and concrete types in the programme, compounding the state migration problem on every deployment.

**Dictionary passing** is Haskell's implementation of type classes and the closest match to what SA's constraint framework naturally produces. At each call site the compiler constructs an explicit record — a dictionary — containing the concrete implementations of the required trait functions. `double<T: Add>(x)` compiles to `double(add_dict, x)` where `add_dict` holds `Int`'s `add` function pointer; the generic function calls through the dictionary. One copy of bytecode exists per generic function, and dictionaries are constructed from statically known information at the call site rather than through runtime lookup. Multiple bounds compose cleanly: dictionaries for `Add + Sub` are two independent records. Against SA's future checker, the fit is strong — dictionaries are the constraint solutions produced by HM(X) made explicit as data, and the query cache stores dictionaries per `(function, concrete_type)` pair naturally. For durable execution the approach is also strong. A paused execution serialises as being inside `double`, not a specialisation. The dictionary is a separate runtime value, not baked into the function identity. Updating `Int`'s `add` means issuing a new dictionary; `double` itself does not change, and existing serialised states survive without migration.

**Witness tables** are Swift's mechanism, and they refine dictionary passing by separating two concerns. A protocol witness table (PWT) records the concrete implementations of protocol requirements; a value witness table (VWT) records how to copy, move, and destroy values of that type, since value sizes differ across types. Generic functions receive both tables alongside the value. Existential types — `any Add` — bundle value, PWT, and VWT in a fixed-size container, allowing values of unknown concrete type to be passed and used uniformly. Whole-module optimisation can then specialise call sites back to monomorphised code where the concrete type is statically known, recovering zero-overhead performance in hot paths. Against SA's future checker, the fit is moderate to strong: the VWT maps onto the kind system and representation polymorphism on SA's roadmap, and the PWT is a dictionary by another name. The query-based architecture suits lazy witness table construction well, though the mechanism requires more infrastructure than dictionary passing alone. For durable execution, witness tables share dictionary passing's stable function identity in serialised state. The risk is the VWT: if a hot update changes a type's memory layout, serialised values of that type require migration because their in-memory form no longer matches the new VWT. Purely behavioural updates — changing a function body without altering representation — are safe.

**Polymorphic specialisation**, proposed by Leroy in the ZINC technical report (1997) and implemented in the MLton whole-programme compiler for Standard ML, occupies the middle ground between full monomorphisation and uniform erasure. Rather than generating a specialisation per type, the compiler analyses the runtime representations of type arguments and generates separate code only where representations genuinely differ. A struct and a scalar warrant different code; two distinct scalar types may share one copy. Against SA's future checker, representation analysis is a natural candidate for a query — "what is the runtime representation of this type?" — though HM(X) would need a representation constraint form to express the resulting obligations. The approach adds complexity to the constraint solver without the clarity of the simpler strategies. For durable execution it is moderately better than full monomorphisation: fewer specialisations mean fewer artefacts to version, but any specialised function still carries a type-derived identity in serialised state, and updates that change representation groupings require migration of paused executions in the affected functions.

**Intensional type analysis**, introduced by Harper and Morrisett ("Compiling Polymorphism using Intensional Type Analysis", POPL 1995, https://dl.acm.org/doi/10.1145/199448.199475) as a typed compilation target, allows compiled code to retain a runtime representation of its type argument and branch on it through a typed `typecase` construct. A single generic function can inspect whether `T` is `Int` or `String` at runtime and take different paths, without separate implementations. The approach breaks parametricity — the guarantee that generic code cannot inspect its type argument — which is an assumption both the HM(X) framework and bidirectional checking rely on for their soundness arguments. This makes it a poor fit for SA's future checker architecture. The durable execution story is mixed: one function appears in serialised state, which is good, but the type tag carried at runtime is part of the serialised value representation. Adding a new type branch in a hot update is safe; removing or reordering branches can corrupt the type tags of paused executions mid-typecase. The dynamic nature of the approach works against static reasoning about which code paths a given update affects.

**Representation polymorphism**, implemented in GHC as levity polymorphism (Eisenberg and Peyton Jones, "Levity Polymorphism", PLDI 2017, https://dl.acm.org/doi/10.1145/3062341.3062357), extends the kind system so that kinds track the runtime representation of a type — boxed pointer, unboxed scalar, SIMD vector, and so on. A function polymorphic over representations can accept both a heap-allocated struct and a stack scalar without boxing either; the compiler selects the appropriate calling convention per instantiation based on the kind. The architectural fit with SA's future checker is strong but costly: the kind system extension is already on SA's roadmap for higher-kinded types, and representation kinds would sit naturally in the same kind checker, with the query-based architecture handling kind queries cleanly. However, GHC has spent years resolving edge cases, and the approach should be considered only once HKTs land. For durable execution the approach is poor. Representation is baked into the calling convention chosen at compile time; a hot update that changes a type's representation invalidates the calling convention of every paused execution involving that type. Since durable execution serialises the full call stack, representation changes require deep migration and are best avoided in a system where live updates are a first-class concern.

**Modular implicits** (White, Bour, Yallop, "Modular Implicits", ML Workshop 2014, https://arxiv.org/abs/1512.01438) express trait bounds as implicit module arguments, using SA's existing sig vocabulary directly. A bound becomes a module parameter resolved implicitly at the call site: `fn double<T: Add>(x: T)` compiles as `fn double(Add_impl: AddModule, x: T)` where `AddModule` is an ML-style module satisfying the `Add` sig, and the compiler finds the right module from those in scope. The architectural fit with SA's future checker is the strongest of any strategy. The implicit module argument is a constraint in HM(X) terms — the solver finds the right module per call site — bidirectional checking propagates the implicit argument top-down, and the query cache stores resolved implicits per call site. This aligns with the observation in [type-system-abstractions.md](type-system-abstractions.md) that sigs and type classes are the same underlying structure. For durable execution the approach is also the strongest. Generic functions have one stable identity in serialised state. Because SA already needs hot-swap semantics for modules — updating an LLM or storage module under a running agent — the module versioning infrastructure applies to trait implementations without additional work. Updating `Int`'s `Add` module does not affect any paused execution inside a generic function; only future calls pick up the new module. The coherence difficulties discussed in Appendix A are the reason OCaml's modular implicits proposal has remained unmerged; SA's same-module restriction and parameterised module wiring address the most common failure modes, as described there.

**Partial evaluation and staging** treat generic functions as two-level programmes: a compile-time level parameterised by the type, and a runtime level that executes. The compiler partially evaluates the compile-time level against known type arguments, reducing the function to code containing only runtime-level computation. Full specialisation subsumes monomorphisation; partial specialisation handles cases where only part of the computation is type-dependent, leaving a stable residual function. Against SA's future checker, the approach aligns conceptually with the query-based architecture — both are forms of lazy computation — and HM(X) constraint solving could in principle drive staging decisions. In practice, implementation complexity is high relative to the benefit at SA's current scale. The durable execution story depends on the degree of specialisation: fully specialised code shares monomorphisation's problems, while a stable partial residual fares better — serialised state points to the residual, which survives updates to the type-specific parts. Maintaining the staging boundary across hot updates without a formal versioning model for staged code is, however, difficult in practice.

SA's chosen strategy is modular implicits, as described in the main text. Dictionary passing is the most direct intermediate step from the current call-site substitution model and may serve as a stepping stone during the transition: the dictionary a call site constructs is the same data that an implicit module argument would carry, so the two representations are convertible. The remaining strategies — monomorphisation, witness tables, polymorphic specialisation, intensional type analysis, representation polymorphism, and partial evaluation — are set aside primarily on durable execution grounds, though each has additional costs noted above.