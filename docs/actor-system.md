# Actor System

SA's actor system extends the module system with identity, mutable state, and cooperative concurrency. A module instantiated as an actor gains a stable string identity, per-instance state, and the ability to run long-lived background tasks that accumulate LLM context across cooperative yield points. The design adds three keywords — `spawn`, `yield`, and `defer` — to a module system that already handles dependency injection, signatures, and context bounding.

The central problem the design solves is how a typed, structured language can allow LLM context to accumulate across multiple function calls to the same actor without abandoning the type safety that makes structured agents tractable. The answer is cooperative reentrancy: an actor function may yield control to other functions on the same actor, all sharing the same execution context, before resuming. Future extensions to the type system — session types and taint tracking — are designed to compose with this model.

## Relationship to the Module System

SA modules are derived in part from 1ML, Rossberg's unification of ML's module and core languages (Rossberg, 2015, "1ML — Core and Modules United"). In 1ML a module is a record, a functor is a function from records to records, and a signature is a type. The distinction between the module language and the core language dissolves.

A parameterised SA module `mod tasks(io: storage.Storage)` is a function from a `Storage` implementation to a record of functions. The observation that drives the actor design is that a parameterised module with per-instance mutable state is structurally equivalent to a class. This is not accidental. Simula 67, the first language to give modules state and instantiation, was itself an Algol extension — a parameterised block with persistent state. Hewitt acknowledged Simula directly as inspiration for the actor model (Hewitt, Bishop, Steiger, 1973, "A Universal Modular ACTOR Formalism"). OOP and ML module systems arrive at the same structure from opposite directions.

The distinction SA draws is:

- A module is a singleton. It has no runtime identity and no mutable state beyond what is passed as a dependency.
- An actor is an instantiated module. It has a stable string identity, per-instance state declared within the module body, and cooperative concurrency via `yield`.

This mirrors the common distinction between a singleton service and a class instance, but grounded in the module system rather than added as a separate construct. See [module-system.md](module-system.md) for the full module system design.

## Spawning Actors

An actor is created with `spawn`, which takes the module type as a type parameter and a string key as its identity. The string key is the grain identity in the Orleans sense (Microsoft Research, "Orleans: Distributed Virtual Actors for Programmability and Scalability", 2014) — it uniquely identifies this instance within the actor registry.

```sa
pub fn main(): String {
    let actor = spawn<SomeActor>("MyActor")
    let value = actor.push_state_with_value()
    return value
}
```

If an actor with the given key already exists in the registry, `spawn` returns a reference to the existing instance rather than creating a new one. This is the virtual actor model: actors are always addressable, activated on first access, and never explicitly destroyed by the caller.

The type of `actor` is a reference to `SomeActor`. Calls on this reference are synchronous from the caller's perspective — the caller blocks until the method returns. Concurrency within the actor is managed by the runtime and exposed only through `yield` and `defer`, not through the calling convention.

## Actor State

A module declares per-instance state with a `state` declaration inside the module body. This state is allocated per actor instance and is not part of the module's public signature.

```sa
mod SomeActor(io: storage.Storage) {
    state counter: Int = 0
    state history: List<String> = []

    pub fn increment(): () {
        counter = counter + 1
    }
}
```

State declarations are not module parameters. A caller spawning `SomeActor` does not supply initial state values — the module defines them with defaults. This preserves encapsulation: the caller is coupled only to the module's public signature, not to its internal data layout. Adding a new state field is not a breaking change to the spawn call site.

This differs from the functional approach of passing state explicitly as a constructor argument. The tradeoff is deliberate. In functional style, all state is explicit at the boundary, which aids auditability. In the actor context, state that changes across the lifetime of the actor — counters, accumulated history, intermediate results — is the actor's private concern. The module parameter list is reserved for injected dependencies (sigs), not for ephemeral internal bookkeeping.

The data is still explicit in the sense that matters for this language: the type of each state field is declared, and the type checker enforces it. Nothing is hidden from the type system, only from callers.

## Context Accumulation and Yield

The core mechanism that allows actors to build up LLM context across multiple calls is `yield`. An actor function that calls `yield` suspends its execution, releases the actor's cooperative lock, and allows other functions on the same actor to run. When those functions complete, the original function resumes from the yield point.

```sa
mod SomeActor {
    pub fn push_state(): () {
        "Did something"!

        while true {
            "Doing something in a loop"!
            yield
        }
    }

    pub fn push_state_with_value(): String {
        "Did something with a value"!
    }
}
```

The `!` injection operator pushes content onto the LLM context stack. Normally, content pushed within a called function is scoped to that call — it is popped when the function returns, consistent with how context scoping works throughout the language. The `yield` point does not change this. Content pushed in the body of `push_state` between two yield points is scoped to that execution slice. Content pushed in `push_state_with_value` is scoped to that call. Both contribute to the shared actor context during the interleaved execution.

The key point is that the resumed function does not see content pushed onto a child context by the interleaved call. Context accumulation across yields is additive in the sense that both functions contribute to the same top-level actor context, but each function's child scope is still popped normally on return. This keeps the existing context scoping model intact and avoids context leaking between function boundaries.

`yield` has type `()`. It is a statement, not an expression. A future extension may allow `yield` to carry a value — `let x = yield` — making actor functions proper generators and closing the gap with session types. That extension is not part of the current design.

## Background Tasks and Defer

A function that yields in an infinite loop never returns in the conventional sense. It is a background task: it runs, contributes context, yields, and resumes repeatedly across the lifetime of the actor. The `defer` keyword starts such a function without blocking the caller.

```sa
pub fn main(): String {
    let actor = spawn<SomeActor>("MyActor")
    defer actor.push_state()
    let value = actor.push_state_with_value()
    return value
}
```

`defer actor.push_state()` starts the background task on the actor. The runtime runs it to its first `yield`, at which point control returns to the caller and execution proceeds to `actor.push_state_with_value()`. The deferred task and subsequent direct calls are interleaved cooperatively, with `yield` as the only preemption point.

`defer` returns `()`. The deferred function's return type is irrelevant since the result is never awaited. The background task runs for the lifetime of the actor unless it exits naturally.

The name `defer` is provisional. The semantics are closer to Go's `go` keyword — launch a concurrent task — than to Go's `defer`, which means run on scope exit. The final name is not settled.

### On-start Protocol

Rather than requiring every caller to `defer` background tasks manually, a module can mark functions to run automatically when the actor is spawned. This is analogous to Erlang's `gen_server` `init/1` callback.

```sa
mod SomeActor {
    on_start fn background(): () {
        while true {
            "Seeding context"!
            yield
        }
    }

    pub fn query(): String {
        "Answer based on context"!
    }
}
```

An `on_start` function runs to its first `yield` during `spawn`, so by the time the caller makes its first call the background context is already seeded. The caller needs no knowledge of the actor's internal lifecycle. `defer` remains available as an escape hatch for dynamically starting background tasks mid-session, but it is not the primary mechanism.

## Execution Model

SA runs on a Tokio async runtime. The cooperative scheduling model maps cleanly: `yield` in the language corresponds to `tokio::task::yield_now()` at the runtime level. Each actor function call becomes a task. The actor's shared context and state are protected by the actor's cooperative lock — only one function runs at a time, and it holds the lock until it returns or yields.

This is the inverse of Orleans' default non-reentrant model. Orleans actors process one message at a time and do not allow reentrancy unless opted in via `[Reentrant]`. SA actors are cooperative by default: reentrancy is permitted, but only at explicit `yield` points chosen by the programmer. The actor author controls exactly where interleaving is safe.

The scheduling policy within a single actor is FIFO. When a function yields, pending calls are dispatched in the order they arrived. This gives deterministic context ordering: the sequence in which content is pushed to the LLM context stack is predictable and reproducible for a given call sequence.

## Relationship to Taint and Trust

The trust lattice described in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) assigns trust levels to modules and enforces that values do not flow from lower-trust to higher-trust contexts without passing through a declared boundary function.

Actors interact with this model at two points. First, a `spawn` call site is a natural trust boundary — the trust level of an actor instance can be declared at spawn time and enforced by the type checker on all subsequent calls. Second, `yield` points are statically identifiable in the source, which makes them tractable checkpoints for taint analysis. At a `yield`, the taint state of all in-scope values in the suspended frame is known. If a tainted value is in scope when a `yield` is reached, the type checker can verify that any interleaved function which touches that value respects the trust boundary.

Taints are currently modelled at the module level — a whole module is either trusted or not. How taint propagates across `yield` boundaries within a single actor, particularly when the suspended frame holds tainted values and the interleaved function is at a different trust level, requires a more precise model. This is an open area of the design. See [Areas Not Fully Formed](#areas-not-fully-formed).

## Relationship to Session Types

Session types (Honda, Vasconcelos, Kubo, 1998, "Language Primitives and Type Discipline for Structured Communication-Based Programming") describe communication protocols as types. A recursive session type such as:

```
Planner = Accumulate(String) . Planner
        | Query               . Plan
```

describes an actor that accepts repeated `Accumulate` messages followed by a terminal `Query` that returns a `Plan`. The recursive branch corresponds to the `while true { yield }` loop; the terminal branch to a non-unit-returning function that fires the LLM.

The `yield`-based design is compatible with session types as a future extension. The `yield` loop in a background task is already the operational equivalent of the recursive branch. Adding session types would make the protocol statically checkable — the type checker would verify that callers send messages in a valid order and that the actor handles all protocol states. This is the typed answer to the limitation that the current design does not enforce call ordering between `defer`-ed background tasks and direct calls.

Session types are not part of the current design because the implementation complexity — equirecursive or isorecursive type checking, dual type synthesis for callers — is high relative to the immediate benefit. The `yield`-based model captures the operational semantics now; session types can layer the static guarantees on top later without requiring a redesign.

## Theoretical Overlap: Session Types, Taints, and CMTT

The three type-theoretic ideas that bear on SA's actor system — session types, taint tracking, and Contextual Modal Type Theory — are not independent inventions. They are different projections of the same underlying structure: the type of a value carries information about the context in which it was produced, and that context constrains where the value can be used. Understanding the overlap matters for SA because the three systems will eventually share the same type checker, and designing that checker to accommodate one well is close to accommodating all three.

### The Shared Foundation

All three connect to modal logic. The necessity modality □A — "A holds in all contexts" — has a computational reading in each system. In CMTT, □A is a closed term independent of any ambient context Ψ; a value of type [· ⊢ A] can be used anywhere because it depends on nothing external. In taint tracking, a sanitised value that has passed through a boundary function is similarly context-independent: it carries no taint and can flow anywhere in the trust lattice. In session types the connection runs through linear logic. Caires and Pfenning (2010, "Session Types as Intuitionistic Linear Propositions") showed that session types correspond precisely to propositions in intuitionistic linear logic under the Curry-Howard correspondence, where the linear modality !A is the resource-sensitive analogue of □A.

The three systems are therefore different instantiations of modal and linear type theory applied to different resources: computational context in CMTT, trust provenance in taint tracking, and communication protocol state in session types.

### CMTT and Taints

The overlap here is the most direct for SA. The CMTT type [Ψ ⊢ A] means a term of type A whose free variables are drawn from context Ψ. If Ψ is parameterised by a trust level — [External ⊢ A] versus [Internal ⊢ A] — then the contextual type directly encodes taint. A value computed from external data has type [External ⊢ A] and cannot be used where [Internal ⊢ A] is required without passing through a boundary function. That boundary function is precisely the substitution operation in CMTT: given a term [External ⊢ A] and a boundary that maps External to Internal, substitution produces [Internal ⊢ A].

The SA `boundary` declaration is therefore a CMTT substitution typed at the trust level. The type checker verifying that tainted values do not cross trust boundaries without a boundary function is verifying the substitution rules of CMTT. Nanevski, Banerjee, and Garg formalised this connection directly in "Verification of Information Flow and Access Control Policies with Dependent Types" (2011), using CMTT-style modalities to express and verify information flow policies.

### Session Types and Taints

A session type describes what can be sent over a channel and in what order. A taint type describes what security level data carries and where it can flow. The two can be combined: a session type carries security labels on each message, specifying not just the type of each message but its trust level. The channel endpoint then becomes a typed boundary in both senses simultaneously — protocol compliance and trust enforcement are unified in the same type. Pottier and Simonet's "Information Flow Inference for ML" (2003) connects information flow directly to the ML type system that SA's module system draws from.

For SA, the `yield` point in an actor sits at the intersection of both systems. It is simultaneously a session type state transition — the protocol moves to the next state as the actor yields — and a potential taint boundary, since tainted values held in the suspended frame may be accessible to the interleaved function. Both constraints apply at the same syntactic point, which is an argument for specifying them in a unified way rather than as two separate passes.

### Session Types and CMTT

Toninho, Caires, and Pfenning (2011, "Dependent Session Types via Intuitionistic Linear Type Theory") combined session types with dependent types, showing that the protocol a channel follows can depend on values sent over it. A session type's protocol state and CMTT's context Ψ are structurally parallel: both are indices carried by a type that evolve as computation proceeds. A session type is essentially a CMTT context that changes with each send or receive rather than being fixed at the point of code generation. The `yield` loop makes this concrete in SA: each yield advances the implicit protocol state, just as each send or receive advances a session type.

### The Implication for SA's Type Checker

All three systems are instances of indexed modal types — types carrying an index that constrains usage. The index is protocol state in session types, trust level in taints, and computational context in CMTT. The underlying machinery is shared: track an index through type derivations, check that index constraints are satisfied at each use site, and verify that transitions between index states are valid.

A type checker designed with indexed types as a first-class concept from the start can accommodate session types, taint, and CMTT-style context bounding through the same infrastructure rather than as separate bolted-on passes. The `yield` point, the `boundary` function, and the `in M` constraint on generated code are all, at the formal level, transitions between indexed modal types. Recognising this is the basis for a unified account.

The full unification — indexed linear modal types where the index simultaneously carries protocol state, security level, and computational context — remains an open research question. The closest published work is Pfenning's group's ongoing programme connecting linear logic, modal types, and session types, but a complete framework incorporating information flow has not been published.

## Areas Not Fully Formed

**Cancellation.** `defer` provides no handle for cancelling a background task. If the calling function exits early or errors, the deferred task continues running. A task handle type — `Task<()>` returned by `defer` — would allow the caller to cancel or await completion. The design for this is open.

**Supervision and failure recovery.** If an actor panics mid-execution or mid-yield, its accumulated context and state are lost. Erlang supervisor trees and Orleans grain lifecycle management handle recovery explicitly. SA has no equivalent. This is significant for the durable execution goal on the language roadmap and will need to be addressed before actors are production-ready.

**Taint propagation across yield.** As noted above, the precise model for how taint state in a suspended frame interacts with interleaved function calls at different trust levels is not yet specified. The cooperative scheduling model makes this tractable — yield points are explicit — but the formal rules are not written.

**`defer` naming.** The keyword `defer` is provisional. Its semantics — launch a fire-and-forget background task — are closer to Go's `go` statement than to Go's `defer`. No final name has been chosen.

**Actor reference types.** The type of the value returned by `spawn<SomeActor>("id")` is a reference to `SomeActor`. Whether this is a nominal type generated per module, a generic `ActorRef<SomeActor>`, or something else is not settled. The current type system is monomorphic; `ActorRef<T>` would require at minimum a limited form of parametric polymorphism or a type alias generated per module.

## Alternatives Considered

### Union Type Messages

The Erlang model uses untyped (or weakly typed) message unions. An actor receives a message that is a union of all possible message types and pattern-matches on it. This avoids the need for a typed interface entirely but sacrifices the call-site type safety that the rest of SA's design depends on. The user of an Erlang actor has no way to know from the type system what messages are valid to send. This was rejected as incompatible with SA's goals.

### Orleans-style Typed Grain Interfaces

Microsoft Orleans types each grain by a C# interface. Each method is an independent, non-reentrant message. The problem for SA is that each call is stateless from the actor's perspective — there is no mechanism to accumulate LLM context across calls without explicitly storing and restoring it as actor state. The context stack would have to be serialised into a state field and deserialised on each call, losing the natural `!` injection model. This was rejected because it fights the language's core execution model.

### CQRS Split

Commands (returning `()`) accumulate context. Queries (returning a non-unit type) fire the LLM against accumulated context and return a typed result. The distinction is encoded in the return type. This is clean and maps well to Orleans. The limitation is that context accumulation across commands must be stored explicitly in actor state — a `List<String>` field or similar. The `yield`-based model is more expressive because accumulation happens naturally through function execution rather than through explicit appends to a list. CQRS remains a valid pattern within the actor model but is not the primary mechanism.

### Context as Message

Rather than the actor accumulating context internally, the caller builds context in its own function scope and passes a snapshot as a single typed message. The actor becomes a pure typed LLM invocation boundary. This is the most conservative design and requires no changes to the context model. It was considered as the minimal approach but rejected because it removes the actor's ability to have its own agency — its own ongoing background reasoning — which is the point of long-lived actor instances in an agent system.

### Recursive Session Types

Session types with explicit recursion (`Planner = String . Planner | Query . Plan`) were considered as the primary mechanism for typed context accumulation. They are the most formally rigorous option. They were deferred rather than rejected: the `yield` design captures the same operational pattern with far less implementation complexity, and session types can be added later as a static verification layer. See [Relationship to Session Types](#relationship-to-session-types).

## Future Ideas

### Behaviour Modules

Erlang's OTP provides behaviour modules — `gen_server`, `gen_statem`, `supervisor` — as templates for common actor patterns. A behaviour module defines a callback protocol; a concrete module implements it and gets the runtime machinery for free. The pattern maps directly onto SA's existing module and signature system.

The three steps required are all expressible through existing mechanisms. Only one new declaration — `behaviour` — is needed to make the pattern ergonomic.

A behaviour is a parameterised module whose parameter is a sig of callbacks:

```/dev/null/gen_server.sa#L1-8
sig Callbacks {
    fn init(): Int
    fn handle_call(state: Int): Int
}

mod GenServer(cb: Callbacks) {
    pub fn start(): () { ... }
    pub fn call(): Int { cb::handle_call(cb::init()) }
}
```

A concrete module declares the behaviour it adopts, implements the required callbacks, and re-exports the behaviour's public functions:

```/dev/null/my_counter.sa#L1-10
mod MyCounter {
    behaviour GenServer

    pub fn init(): Int { 0 }
    pub fn handle_call(state: Int): Int { state + 1 }

    pub use GenServer::start
    pub use GenServer::call
}
```

The `behaviour GenServer` declaration does three things the type checker can verify statically. First, it checks that `MyCounter` implements all functions declared in `GenServer`'s sig param — a check the type checker already performs when validating wired modules against signatures. Second, it instantiates `GenServer(MyCounter)` implicitly, equivalent to the external wiring currently done at the entry module. Third, it makes `GenServer`'s public functions available for `pub use` re-export.

This is not circular. `GenServer` depends on the abstract `Callbacks` sig, not on `MyCounter` concretely. `MyCounter` satisfies `Callbacks`. The instantiation `GenServer(MyCounter)` is valid because the type checker confirms `MyCounter : Callbacks`.

The closest analogues are Haskell typeclass instances (`instance Callbacks MyCounter where ...`) and Rust `impl` blocks that derive trait methods. SA's version operates at the module level, so the behaviour carries full module machinery — state, background tasks, lifecycle — rather than only method dispatch.

The practical change required is that the vtable resolution in the compiler currently handles wiring driven from the entry module via `WiringSite` and `ModuleBinding` declarations. A `behaviour` declaration would be a new resolution site driven from within the declaring module rather than from outside. The mechanism is the same; only the source of the wiring instruction changes.

## See Also

- [module-system.md](module-system.md) — SA module system design including signatures and dependency injection
- [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — taint tracking and trust lattice
- [durable-execution.md](durable-execution.md) — durable execution roadmap
- Rossberg, A. (2015). "1ML — Core and Modules United." ICFP 2015. https://people.mpi-sws.org/~rossberg/1ml/
- Hewitt, C., Bishop, P., Steiger, R. (1973). "A Universal Modular ACTOR Formalism for Artificial Intelligence." IJCAI 1973.
- Honda, K., Vasconcelos, V., Kubo, M. (1998). "Language Primitives and Type Discipline for Structured Communication-Based Programming." ESOP 1998.
- Microsoft Research. (2014). "Orleans: Distributed Virtual Actors for Programmability and Scalability." https://www.microsoft.com/en-us/research/publication/orleans-distributed-virtual-actors-for-programmability-and-scalability/
