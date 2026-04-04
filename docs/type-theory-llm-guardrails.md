# Type Theory, Modules, and LLM Guardrails

Language models are stochastic. Given the same prompt twice, they may return different results. Building reliable software on top of them therefore requires guardrails that go beyond hoping the model behaves well. This document explores how ideas from type theory — in particular contextual modal type theory, dependent types, refinement, and information flow — apply to constraining what a language model can generate in SA.

## The Core Problem

SA allows a language model to generate values at runtime, constrained by a return type. The natural extension is allowing the model to generate *functions* — not just data. A function signature becomes the return type, and the model fills in the body. The problem is that a function body has an unbounded call graph. Without constraints, the model could generate a call to anything, including things that do not exist or should not be called.

Two orthogonal constraints are needed. First, the *vocabulary* of functions the generated code may call must be bounded. Second, the *values* the generated function may return must satisfy invariants beyond mere structural type correctness.

The second problem is prompt injection. A language model processes data from external sources — user input, web content, database records — and an attacker can embed instructions in that data. The model, unable to distinguish instructions from data, follows them. This is not a failure of the model's intelligence; it is a consequence of the absence of any formal separation between data and instructions in current systems. The attacker does not need to compromise the model or the host program. Placing "ignore previous instructions" in a document the model reads is sufficient.

These two problems are related. Both arise from the absence of a formal boundary between what the model can *do* and what it can be *made to do*. A type system that bounds the call vocabulary also limits the damage an injected instruction can cause — the model cannot be instructed to call functions outside its module context. Taint tracking on values adds a second layer, preventing attacker-controlled data from reaching the model as instructions without passing through a human-written parser.

## Modules as Environments

The cleanest way to bound the call vocabulary is to make the *environment* of a function part of its type. A module defines a set of functions. A generated function declared `in M` can only call what M exports. This is not a runtime check — it is a property of the type.

```/dev/null/example.sa#L1-6
mod M {
    fn some_fun(x: Int): Int
    fn other_fun(x: Int): Int
}

fn generate_strategy(ctx: Context): ((number: Int): Int) in M
```

The formal reading of `((number: Int): Int) in M` is a *closed term type* — a function of type `Int -> Int` whose free variables are drawn exclusively from M. The LLM, when generating the body, receives M's signature as context. The runtime verifies that the generated body's free variables are a subset of M's exports before execution. This check is decidable.

This maps directly to **Contextual Modal Type Theory** (CMTT), developed by Nanevski, Pfenning, and Pientka in 2008. CMTT introduces the notation `[Ψ ⊢ A]` for a term of type A in context Ψ. The `in M` clause is exactly Ψ. The full formal reading of the signature above is `[M ⊢ (Int -> Int)]`.

The application here is a reversal of CMTT's original purpose. CMTT was designed for meta-programming over a fixed object language, where the context is known statically. In SA the context (M) is fixed by the programmer and the term (the function body) is generated at runtime by the model. The theory applies but the direction is novel.

## What CMTT Gives SA

Beyond bounding the call vocabulary, CMTT provides a formal account of several properties central to SA's design.

**Staged computation.** CMTT was originally designed for multi-stage programming — reasoning about code that generates code. The `□A` modality means "a closed term of type A that can be evaluated later". This maps directly to SA's execution model where an LLM generates a function at one stage that is executed at another. The type system can distinguish a value of type `A` available now from a value of type `□A` — code that produces an `A` — to be run later. This gives a formal account of the distinction between the LLM generation phase and the execution phase.

**Substitution as a typed operation.** In CMTT, substituting a context is a typed operation. If you have a term `[M ⊢ A]` and a concrete implementation of M, substitution produces an `A`. This means swapping the module a generated function runs against is type-safe — you can generate a function against an abstract module signature and later instantiate it with a concrete implementation, with the type checker verifying the implementation satisfies the signature. A function generated against a mock module in testing runs against the real module in production without modification.

**Explicit context manipulation.** CMTT makes context explicit and manipulable as a typed thing rather than an implicit ambient environment. This gives a formal basis for context extension (adding to what a generated function can see), context restriction (narrowing what a later stage can access), and context passing (handing a context between parts of the program as a value). SA already threads context explicitly; CMTT is the formal justification for why that design is correct.

**Separation of object and meta levels.** CMTT maintains a strict separation between the level at which programs run and the level at which programs are constructed. Prompt injection is an attempt to collapse that separation — making object-level data behave as meta-level instructions. CMTT's type discipline enforces that data at the object level cannot become code at the meta level without an explicit typed lifting operation. The `in M` constraint is the mechanism that keeps generated code from escaping into the host program.

The full picture CMTT offers is a type-theoretic account of the entire LLM generation pipeline:

```/dev/null/pipeline.txt#L1-6
Meta level:    SA program, module definitions, type checker
               ↓  □-introduction: LLM generates an AST
Object level:  [M ⊢ A]  — generated term typechecked in context M
               ↓  substitution: instantiate M, execute
Value level:   A  — runtime result
```

Each arrow is a typed operation. The type system reasons about all three levels uniformly.

## First-Class Modules

OCaml's module system is the most direct precedent in mainstream languages. OCaml **functors** parameterise a module by another module — they are functions from module to module — and module types (signatures) act as constraints on functor parameters. A functor argument `(M : SIG)` restricts the produced module to calling only what SIG exposes.

**1ML**, developed by Andreas Rossberg, unifies the module and core language of ML entirely, making modules first-class values. This is the cleanest formal treatment of the idea and is worth reading before committing to a syntax for SA. See [the 1ML paper](https://people.mpi-sws.org/~rossberg/1ml/).

Standard ML and F# share the same module tradition. The practical lesson from all of them is that module types (signatures) and value types are better kept unified than treated as separate systems. A function signature `((number: Int): Int) in M` is simply a type, and M is simply a value of module type.

## Types and Invariants

A type describes the *shape* of a value. An invariant describes something *meaningful* about it — `x > 0`, a list is sorted, a result satisfies some domain property. These are distinct concerns and both are needed.

In dependent type theory an invariant is encoded as a **refinement type**, formally a Σ-type (sigma type, or dependent pair). The type `{ x: Int | x > 0 }` is `Σ(x: Int). P(x)` — a pair of an integer and a proof that the integer satisfies P. The proof is part of the value.

For generated functions the same principle applies. An invariant `result(0) == 0` over a generated function `f: Int -> Int` is encoded as `Σ(f: Int -> Int). f(0) == 0`. A value of this type is the function paired with evidence of the property. At runtime, carrying a formal proof is impractical, so the invariant falls back to a checked contract — but the *semantics* are those of dependent types.

This is the foundation of **liquid types** (Rondon, Kawaguchi, Jhala, 2008), which use SMT solvers to discharge refinement proof obligations automatically. The programmer writes `{ x: Int | x > 0 }` and the solver does the work. Liquid types occupy a useful point on the spectrum between plain contracts and full theorem proving. [The original paper](https://dl.acm.org/doi/10.1145/1375581.1375602) is the primary reference.

## The Guardrail Spectrum

Three mechanisms operate at different levels and are not in competition.

The module constraint limits what the LLM can *express*. By restricting the call vocabulary to M, the space of programs the model can generate is bounded before any code runs. This is the strongest intervention because it shapes the model's output distribution rather than reacting to it.

The type constrains what the LLM can *return* structurally. SA already does this — a function declared to return `Int` will have its output parsed and validated as an integer. Extending this to function types is the subject of the rest of this document.

The runtime contract checks whether a returned value or function satisfies an *invariant* beyond structure. For generated functions this means running the function on witnesses and asserting properties. This is property-based testing baked into the language semantics, and it is necessary because types alone cannot express all meaningful constraints.

Full refinement proof obligations in the style of the **B method** — where every refinement step is formally proved correct before execution — are probably too strong for a language whose primary output is LLM-generated code. The programmer cost of writing proof obligations for every generated function would be prohibitive. The right level is contracts at runtime with the *semantics* of dependent types, not mechanised proofs.

## Taint Tracking and Prompt Injection

Prompt injection is the principal security threat to LLM-based systems. An attacker embeds instructions in data the model processes — a document, a web page, a database field — and the model follows those instructions as if they were from the programmer. Module constraints and types bound what the model can *generate*, but they do not address what it can be *made to generate* by attacker-controlled input.

Taint labels on function signatures offer a complementary defence. A function that returns data from an untrusted source declares this in its return type using the `+` qualifier syntax. The type checker propagates the label through the generated AST — since the LLM generates an AST that is typechecked before execution, the type checker has full visibility of the call graph and propagates labels exactly as it would for any other type. A `String + Tainted` cannot be passed where `String` is expected without an explicit boundary crossing. The override is declared with `unsafe`, making the breach visible and auditable.

```/dev/null/example.sa#L1-12
mod External {
    fn fetch_user_input(): String + Tainted
    fn fetch_web_content(url: String): String + Tainted
    fn get_status(url: String): Int
}

mod Parsers {
    fn parse_int(s: String + Tainted): Option<Int>
    fn parse_command(s: String + Tainted): Option<Command>
}

fn handle(ctx: Context): () in External {
    let raw = fetch_user_input()
    let n = Parsers::parse_int(raw)
}
```

Note that `get_status` returns a clean `Int` — taint is per function signature, not per module. The granularity is at the value level, declared explicitly by the programmer in the module signature alongside all other type information.

### Label Syntax

Labels are written as `T + L` where `T` is a base type and `L` is a label. Multiple labels stack naturally:

```/dev/null/example.sa#L1-3
fn fetch(): String + Tainted + External
fn process(s: String + Tainted): String
fn log(s: String + Audited): ()
```

`T + L` is not symmetric with `T + T` — `String + Tainted` is valid because `Tainted` is a label, but `String + Int` is a type error since both are base types. Labels occupy a separate namespace from types. The `+` syntax reads as a qualification rather than a union, which matches the semantics: a `String + Tainted` is still a `String`, it simply carries provenance information. The type checker's propagation rule is that if any argument to a function carries label `L`, the return type carries `L` unless the function signature explicitly drops it via a boundary declaration — the join operation from the trust lattice expressed in syntax.

### The Trust Lattice

Module relationships form a partial order — a trust lattice — that the type checker enforces. A value cannot flow from a lower-trust context to a higher-trust one without passing through a declared boundary function. The boundary is a typed, auditable declaration:

```/dev/null/example.sa#L1-7
trust External < Internal

boundary parse_input: External -> Internal {
    fn parse(s: String + Tainted): Option<String>
}

unsafe boundary raw_pass: External -> Internal {
    fn forward(s: String + Tainted): String
}
```

Direct flow between modules that violates the lattice order is a type error. The `unsafe boundary` declaration is the only sanctioned escape hatch, and every call site through it is findable by a straightforward query over the AST. This is object-capability security expressed through the module system — a module's authority is exactly what its position in the lattice grants, no more.

### Label Polymorphism

Generic functions must propagate taint labels through type variables. Without this, every generic function would require multiple variants for each combination of taint levels. With the `+` syntax, label polymorphism extends naturally — the label is a type parameter that propagates through the type checker:

```/dev/null/example.sa#L1-1
fn add<ℓ>(a: Int + ℓ, b: Int + ℓ): Int + ℓ
```

This is the same mechanism as `Option<T>` or `List<T>` already in SA's type system. The label is a type parameter and propagates through the type checker in the same way. This will be needed as SA's standard library grows.

### What Is Taken from Information Flow Types

Information flow type theory, specifically the **Decentralised Label Model** (DLM) developed by Myers and Liskov, is the theoretical foundation. However SA does not need the full apparatus. What is needed:

`T + Tainted` as a qualified type carrying provenance. The rule that `T + Tainted` cannot be passed where `T` is expected without a boundary crossing. Label propagation through function signatures in the type checker. Label polymorphism for generic functions.

What is not needed: PC labelling for implicit flows through control structure (a known open problem across the entire field, not specific to SA), full noninterference proofs (the practical guarantee from `Tainted<T>` is sufficient), and declassification policies (the boundary declaration mechanism handles this directly).

The trust lattice and boundary declarations give SA's capability model a formal provenance-tracking layer. The combination — CMTT bounding the call vocabulary, taint labels tracking data provenance — provides stronger security guarantees than either mechanism alone.

### Limitations

Taint tracks *data* provenance but not *control flow* provenance. A compromised model operating within a tainted module could call legitimate functions in a harmful sequence with clean-typed arguments, all of which would pass the type checker. Implicit flows through control structure — branching on tainted data and writing to a clean variable — are not caught by value-level taint. This is an unsolved problem in information flow type theory generally and is not specific to SA. Implicit flows become a concern primarily in sophisticated adversarial scenarios; current prompt injection attacks rely on raw untrusted text reaching the model directly, which `Tainted<T>` addresses.

Parsers that perform taint removal must be human-written and outside LLM generation scope. If the model can influence which parser is called the protection is weakened.

## The B Method and Formal Verification

The B method, developed by Jean-Raymond Abrial, is the precedent for formal verification of safety-critical software. The Météor line (Paris Métro Line 14, opened 1998) had its safety-critical software specified and verified using B. The system has operated without a software bug in production since opening. Abrial later applied the same approach to the Calcutta Metro and parts of the London Jubilee Line extension.

B works by **refinement**: an abstract machine is specified with invariants, then mechanically refined in stages down to executable code. Each refinement step is proved correct. By the time code exists, correctness is guaranteed by construction. Abrial's book [*The B-Book*](https://www.cambridge.org/core/books/bbook/D0FA939B9F72E49DBA14BA0F68491118) (1996) is the primary reference. His later [*Modeling in Event-B*](https://www.cambridge.org/core/books/modeling-in-eventb/F39FF5F1B60F0AA585718B8E6A4F9DD7) (2010) covers the successor formalism.

B's foundations are set-theoretic (based on ZF set theory) rather than type-theoretic. The connection to type theory runs through the **Curry-Howard correspondence** — a refinement proof in B is, structurally, a proof that a term inhabits a dependent type. Coq and Agda make this explicit; B treats it operationally through proof obligations discharged by the Atelier B tool.

## Relevant Type Theory

**TAPL** (Pierce, 2002) covers subtyping and existential types (Ch. 24), which are the foundational material. Existential types are the formal model for modules-as-values: a module `{ type t; val f: t -> t }` is modelled as `∃t. { f: t -> t }`.

**ATAPL** (Pierce et al., 2004) covers modal type theory in Ch. 2 (Pfenning and Davies, "A Judgmental Reconstruction of Modal Logic") — the foundation on which CMTT builds — but predates CMTT itself by four years. Chapter 8 covers module type theory directly.

**PFPL** (Harper, 2016) is the most current single-volume treatment, reflecting the Carnegie Mellon school that produced both CMTT and much of the dependent type work. It covers modal types, phase distinctions, and refinement. Freely available on Harper's website.

**Contextual Modal Type Theory** (Nanevski, Pfenning, Pientka, 2008) is the direct theoretical basis for the `in M` constraint described above. [ACM link](https://dl.acm.org/doi/10.1145/1352582.1352591). Pientka's language **Beluga** implements CMTT and is worth reading for its concrete syntax, even if the language itself is not practical for SA. [Beluga project](http://complogic.cs.mcgill.ca/beluga/).

**Lean 4** is currently the most practical language in which these ideas can be explored directly. It shares the same theoretical foundation as Coq (Calculus of Constructions with dependent types) but is designed as a general-purpose programming language as well as a theorem prover. **LeanDojo** (Yang et al., 2023) uses LLMs to generate Lean proofs — the direct dual of what SA is doing — and is directly relevant reading.

**Information flow and security types.** Myers and Liskov's DLM and the Jif implementation are the primary sources. Myers maintains a current publication list at [https://www.cs.cornell.edu/andru/pubs-topic.html#infosec](https://www.cs.cornell.edu/andru/pubs-topic.html#infosec). Volpano, Smith and Irvine (1996) gave the first type-theoretic account framing information flow as a type system property.

## Diagram

```/dev/null/guardrails.txt#L1-20
                     ┌─────────────────────────────────┐
                     │         SA Function Call         │
                     └────────────────┬────────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              │                       │                       │
     ┌────────▼────────┐   ┌─────────▼──────────┐  ┌────────▼────────┐
     │ Module Constraint│   │   Type Constraint  │  │Runtime Contract │
     │  (call vocab)   │   │  (return shape)    │  │  (invariants)   │
     │  [M ⊢ A]        │   │  Int, Tainted<T>   │  │  x > 0, f(0)==0 │
     └─────────────────┘   └────────────────────┘  └─────────────────┘
     strongest: shapes      includes taint labels   weakest: fires
     LLM distribution       checked on generated    after generation
                            AST before execution

     Trust Lattice:
     UserInput < External < Internal
                    ↕                ↕
             unsafe boundary    boundary fn
             (auditable)        (typed parser)
```

## See Also

### Papers

- [1ML paper](https://people.mpi-sws.org/~rossberg/1ml/) — Rossberg, 2015
- [CMTT paper](https://dl.acm.org/doi/10.1145/1352582.1352591) — Nanevski, Pfenning, Pientka, 2008
- [CMTT conference paper](https://www.cs.cmu.edu/~fp/papers/cmtt05.pdf) — Pfenning, 2005 (free PDF)
- [CMTT journal paper](https://www.cs.cmu.edu/~fp/papers/tocl07.pdf) — Nanevski, Pfenning, Pientka (free PDF)
- [Liquid Types paper](https://dl.acm.org/doi/10.1145/1375581.1375602) — Rondon, Kawaguchi, Jhala, 2008
- [LeanDojo](https://leandojo.org) — Yang et al., 2023
- [Jif project](https://www.cs.cornell.edu/jif/) — Myers & Liskov, information flow types
- [Myers information flow publications](https://www.cs.cornell.edu/andru/pubs-topic.html#infosec) — Cornell
- Kennedy, A. (1994). "Programming Languages and Dimensions" — original units of measure paper; later implemented in F#. The type erasure and label polymorphism properties are directly applicable to SA's `+` label syntax
- [F# Units of Measure](https://learn.microsoft.com/en-us/dotnet/fsharp/language-reference/units-of-measure) — Microsoft, F# language reference

### Books

- Pierce, B. C. (2002). *Types and Programming Languages*. MIT Press. — foundational; existential types Ch. 24, subtyping Ch. 15
- Pierce, B. C. et al. (2004). *Advanced Topics in Types and Programming Languages*. MIT Press. — modal type theory Ch. 2; module type theory Ch. 8
- Harper, R. (2016). *Practical Foundations of Programming Languages* (2nd ed.). Cambridge University Press. — modal types, refinement, phase distinctions. [Free PDF](https://www.cs.cmu.edu/~rwh/pfpl/)
- Abrial, J-R. (1996). *The B-Book: Assigning Programs to Meanings*. Cambridge University Press. [Cambridge Core](https://www.cambridge.org/core/books/bbook/D0FA939B9F72E49DBA14BA0F68491118)
- Abrial, J-R. (2010). *Modeling in Event-B: System and Software Engineering*. Cambridge University Press. [Cambridge Core](https://www.cambridge.org/core/books/modeling-in-eventb/F39FF5F1B60F0AA585718B8E6A4F9DD7)

### Project Files

- [ideas.md](../src/structured-agent/ideas.md) — SA language design notes
- [struct-types.md](struct-types.md) — current type system implementation
- [Beluga](http://complogic.cs.mcgill.ca/beluga/) — Pientka
