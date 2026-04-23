# Binding and Scope

The typed AST tracks bound types on every node but delegates scope entirely to the runtime. Variable names are strings, looked up by walking a chain of hash maps at execution time. This document describes a design that moves scope into the typed AST as explicit binding sites, lowering to numbered slots at the IL level. The change unifies value bindings, module parameters, and type parameter witnesses under one mechanism, replacing several ad-hoc codegen paths with a single coherent model. It is also the foundation on which contextual types, taint labels, and session types — all described in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) and [actor-system.md](actor-system.md) — can be built without further structural revision.

## The Current Problem

Every variable in the current pipeline is identified by a plain string. The typed AST carries that string from elaboration through codegen. The IL carries it in every instruction. The VM stores a `HashMap<String, ExpressionResult>` per scope frame, and variable lookup walks a linked chain of frames up to the nearest function boundary. The `Context` struct in `src/structured-agent-interpreter-runtime/src/context.rs` is that chain:

```/dev/null/context-today.rs#L1-8
pub struct Context {
    parent: Option<Box<Context>>,
    variables: HashMap<String, ExpressionResult>,
    is_scope_boundary: bool,
    return_value: Option<ExpressionResult>,
    events: Vec<Event>,
    runtime: Arc<dyn RuntimeService>,
    agent_handle: AgentHandle,
}
```

The same structure serves two distinct purposes: variable scoping and event accumulation. Events are the mechanism by which functions build conversational context for LLM calls, walking the parent chain to collect everything accumulated by callers. The two concerns are entangled in one type.

Module parameters introduce a second ad-hoc layer. When a function is imported from a parameterised module, the compiler needs to pass the concrete module implementation into the callee. It does this by computing `leading_count` — the difference between the total argument count and the declared parameter count — and treating the excess leading arguments as module references loaded into named variables. The callee's body then calls `CallIndirect { module_param: String, fn_name: String, ... }`, reading the module reference back out of the context by name. The same string is produced by two independent symbol-table re-examinations at elaboration time: `resolve_use_param_bindings` on the caller side and `resolve_function_alias_via_param` on the callee side. Nothing in the type system connects them; they agree only by convention.

Type parameters compound the problem. `TypeEnvironment` stores type params as `HashMap<String, ()>` — a name-presence check that discards bound information entirely after elaboration. When a bounded type parameter `T: Add` is instantiated, the typechecker can verify that the concrete type satisfies `Add`, but there is no binding site carrying a witness slot for the `Add` implementation. Trait method dispatch inside the generic function body therefore has no place to thread the impl through to the callee. This is why trait/impl support remains stubbed and all trait tests carry `#[ignore = "impl/trait refactor in progress"]`.

These three problems — string-keyed variable scope, ad-hoc module parameter threading, and discarded type parameter witnesses — share the same root cause. Scope is not represented in the program; it is reconstructed at runtime from string names.

## Binding IDs in the Typed AST

The fix begins at elaboration. Name resolution, which currently dissolves into string comparisons scattered across the typecheck passes, becomes a single dedicated phase that assigns a globally unique `BindingId` to every binding site. A binding site is any point in the program that introduces a name: a function parameter, a `let` or assignment statement, a type parameter, or a module parameter. The `BindingId` is a monotonically incrementing `u32`. The source name is preserved alongside it for diagnostics and display but plays no further semantic role.

```/dev/null/binding-id.rs#L1-4
struct BindingId(u32);

struct Parameter {
    id: BindingId,
    name: String,      // retained for error messages only
    param_type: Type,
}
```

Every reference to a name in the typed AST carries the `BindingId` of the binding site it was resolved to, not a string. `Expression::Variable` becomes a reference to a specific binding, not a lookup by name. Two references with the same `BindingId` provably refer to the same binding. Two bindings named `x` in nested scopes have different IDs and are never confused.

Type parameters gain explicit binding structure. Today `TypeParam { name: String, bounds: Vec<Type> }` carries the type's name and a list of bound type names — enough to check satisfaction but not to thread witnesses through to codegen. With binding IDs, each type parameter binding carries a witness slot alongside its bounds:

```/dev/null/typeparam-binding.rs#L1-6
struct TypeParamBinding {
    id: BindingId,
    name: String,
    bounds: Vec<TraitRef>,
    witness_slots: Vec<BindingId>,  // one per bound, holds the impl at call sites
}
```

The `Function` in the typed AST separates its three kinds of parameter explicitly:

```/dev/null/function.rs#L1-7
struct Function {
    name: String,
    module_params: Vec<ModuleParamBinding>,
    type_params: Vec<TypeParamBinding>,
    value_params: Vec<Parameter>,
    return_type: Type,
    body: FunctionBody,
}
```

Module params, type param witness slots, and value params are structurally distinct. No counting heuristic determines which is which. The `leading_count` calculation in `invoke_function` and both `resolve_use_param_bindings` and `resolve_function_alias_via_param` become unnecessary: the binding structure carries all the information they were reconstructing at runtime.

## Slot-Indexed IL

The binding IDs established in the typed AST drive a second change at the IL level. The bytecode compiler, when lowering a function, allocates a numbered slot for each binding in the function's scope. Slots are dense indices into a flat array; each function's compiled form carries a slot table in its header declaring the type and kind of each slot.

```/dev/null/slot-table.txt#L1-10
CompiledFunction header:
  Slot(0) -> return place       ReturnSlot
  Slot(1) -> module param "io"  ModuleParam
  Slot(2) -> type witness "T"   TypeWitness
  Slot(3) -> value param "x"    ValueParam
  Slot(4) -> local "$tmp0"      Local
  Slot(5) -> local "result"     Local
```

Slot(0) is reserved for the return value, following the convention used by rustc's MIR. All other instructions address slots by index. The string names in the slot table are retained for debuggers and diagnostic output but are not used for lookup.

The instructions that currently carry string names — `LdcInt { dest: String }`, `Mov { dest: String, src: String }`, `CallBytecode { params: Vec<String> }` — change their payload to `Slot`. `CallIndirect { module_param: String, fn_name: String }` changes `module_param` from a string requiring a runtime variable lookup to a `Slot` index into the current frame. The VM reads the module reference by direct array index rather than by string hash map traversal.

Several instructions that exist solely to manage the string-keyed scope chain are eliminated. `Decl` was needed because the same string name could appear in nested scopes and the VM needed to know when a new binding shadowed an outer one. With unique names in the IL — each binding in a function carries a different slot index — there is nothing to shadow. `Drop` performed the symmetric cleanup. Both disappear. `CtxChild { is_scope_boundary: false }` and its matching `CtxRestore` were emitted around if and while bodies to create a new scope frame for variables declared inside them. With slot-indexed locals, variables declared inside a while loop body occupy fixed slots in the function's frame; each loop iteration overwrites the same slot, which is correct behaviour. The inner-scope context manipulation is unnecessary and is eliminated.

What remains of `CtxChild` and `CtxRestore` is the function-call boundary case, `CtxChild { is_scope_boundary: true }`, which today has two jobs. After the split described in the next section, this reduces to one.

```/dev/null/il-comparison.txt#L1-17
-- today
Decl     { name: "x" }
LdcInt   { dest: "x",  value: 42 }
Decl     { name: "y" }
Mov      { dest: "y",  src: "x" }
Drop     { name: "x" }
CtxChild { is_scope_boundary: false }
...
CtxRestore

-- with slots
LdcInt   { dest: Slot(4),  value: 42 }
Mov      { dest: Slot(5),  src: Slot(4) }
```

## Splitting the Context

`Context` currently serves two purposes that must be separated before the slot-indexed frame model is coherent.

The first purpose is variable storage and scope. With slots, this becomes a flat `Vec<Option<ExpressionResult>>` allocated once per function call and sized to the slot table. There is no parent chain for variable lookup because every slot belongs unambiguously to the current function's frame. A sub-scope within a function — an if branch, a while body — shares the enclosing function's frame; it does not create a new one.

The second purpose is event accumulation. Events are values injected into context using `CtxEvent` and later read by the LLM infrastructure to build conversational history. A function's events must include those of its callers, so the event structure is genuinely hierarchical and the parent chain is correct for it. This purpose must survive the redesign.

The two concerns separate into two types:

```/dev/null/split-context.txt#L1-16
Frame {
    slots: Vec<Option<ExpressionResult>>,
}

EventContext {
    parent: Option<Box<EventContext>>,
    events: Vec<Event>,
    return_value: Option<ExpressionResult>,
    runtime: Arc<dyn RuntimeService>,
    agent_handle: AgentHandle,
}

VMState {
    pc: usize,
    frame: Frame,
    event_ctx: EventContext,
}
```

A function call allocates a new `Frame` sized to the callee's slot table and creates a new `EventContext` child. On return, the caller's `Frame` is restored and the `EventContext` either remains as a child node in the event chain — so the caller can see the callee's events — or merges upward depending on the event collection semantics, which are unchanged from today.

The `is_scope_boundary` flag disappears with it. There was previously one kind of boundary for variable scoping (function call) and a second kind (if/while blocks) that shared the variable chain but separated scope. With slots, only function calls create new frames; there is no second kind. `CtxChild` and `CtxRestore` as IL instructions are removed; the `invoke_function` mechanism in the VM handles frame allocation and `EventContext` creation directly.

The following diagram shows the structure before and after:

```/dev/null/diagram.txt#L1-35
Before:

  call stack
  ┌───────────────────────────────┐
  │ Context (caller)              │
  │  variables: { "x": .., .. }  │
  │  events: [..]                 │
  │  is_scope_boundary: true      │
  │  parent: None                 │
  └──────────────┬────────────────┘
                 │ CtxChild(true)
  ┌──────────────▼────────────────┐
  │ Context (callee fn)           │
  │  variables: { "y": .., .. }  │
  │  events: [..]                 │
  │  is_scope_boundary: true      │
  └──────────────┬────────────────┘
                 │ CtxChild(false)
  ┌──────────────▼────────────────┐
  │ Context (if body)             │
  │  variables: { "z": .., .. }  │
  │  events: []                   │
  │  is_scope_boundary: false     │
  └───────────────────────────────┘

After:

  Frame (caller)           EventContext (caller)
  [ slot0, slot1, .. ]  ←─ events: [..]
                            parent: None
                                │
  Frame (callee)           EventContext (callee)
  [ slot0, slot1, .. ]  ←─ events: [..]
                            parent: ──────────►(caller EventContext)
  (if body uses same
   callee Frame slots)
```

## Relation to Future Type System Features

The three type system directions documented elsewhere all require explicit scope context as a typed structure, not as a runtime convention.

Contextual Modal Type Theory, described in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md), introduces the notation `[Ψ ⊢ A]` for a term of type A whose free variables are drawn from context Ψ. The `in M` constraint on generated functions — bounding what an LLM-generated function may call — is exactly this. Verifying the constraint means checking that every variable reference in a generated term resolves to a binding in Ψ. Without binding IDs in the typed AST, that check must scan string names at runtime. With binding IDs, it is a structural property of the typed term, checkable before execution.

Taint labels, also described in [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md), require label polymorphism: `fn add<ℓ>(a: Int + ℓ, b: Int + ℓ): Int + ℓ` where `ℓ` is a label variable. A label variable is a type parameter whose binding site is the function signature. Without explicit type parameter binding sites, label polymorphism degrades to the same `HashMap<String, ()>` presence check that currently discards bound information.

Session types, described in [actor-system.md](actor-system.md), describe communication protocols as types whose index advances with each `yield`. A protocol state index is a type-level value threaded through a binding scope. The connection between session type state transitions and CMTT context manipulation is direct: both are instances of indexed modal types where the index lives in a scope. The event chain in `EventContext` is the operational precursor to session type state — it accumulates the same information a session type would track, without the static guarantees. Adding session types means annotating the yield points with type-level indices, which requires those indices to be first-class bindings in the AST.

All three converge on the same requirement. A type checker for indexed modal types needs one mechanism — a binding site carrying an index — not one mechanism per feature. This is the point made explicitly in [actor-system.md](actor-system.md):

> A type checker designed with indexed types as a first-class concept from the start can accommodate session types, taint, and CMTT-style context bounding through the same infrastructure rather than as separate bolted-on passes.

Binding IDs are the substrate that makes indexed types first-class.

## See Also

- [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — CMTT, taint labels, and the `in M` constraint
- [actor-system.md](actor-system.md) — session types, yield, and event accumulation
- [constrained-generics.md](constrained-generics.md) — trait bounds and modular implicits dispatch
- [module-parameter-implementation.md](module-parameter-implementation.md) — the `CallIndirect` and leading-param mechanism this replaces
- `src/structured-agent-typed-ast/src/lib.rs` — current typed AST node definitions
- `src/structured-agent-il/src/instruction.rs` — current IL instruction set
- `src/structured-agent-interpreter-runtime/src/context.rs` — current `Context` definition
- `src/structured-agent-vm/src/vm.rs` — `invoke_function` and `leading_count`
- `src/structured-agent-typecheck/src/synthesize.rs` — `TypeEnvironment` and `type_params: HashMap<String, ()>`
- `src/structured-agent-typecheck/src/db.rs` — `resolve_use_param_bindings` and `resolve_function_alias_via_param`

---

## Appendix: Prior Art

### GHC

GHC (https://gitlab.haskell.org/ghc/ghc) assigns a `Unique` — a `u32` — to every `Name` at renaming time. The `Name` type carries both the `Unique` and a human-readable `OccName`. Equality is decided by `Unique` comparison; the string is never used for semantic purposes after renaming. GHC Core, the typed intermediate representation, carries explicit `TyBinder` nodes in the type of every polymorphic function: `forall a. a -> a` encodes the binder for `a` as a node, not as a string in a list. Type variable references carry the same `Unique` as their binder. This is the most direct precedent for the binding ID approach described above. See the GHC Commentary: https://gitlab.haskell.org/ghc/ghc/-/wikis/commentary/compiler/name-type.

### rustc

rustc (https://github.com/rust-lang/rust) uses a two-phase representation that maps precisely to SA's pipeline. In the HIR (High-level IR), variables are identified by `HirId = (DefId, ItemLocalId)`. When lowering to MIR (Mid-level IR), variables become numbered locals: `_0` is the return place, `_1` through `_n` are parameters, and further indices are compiler-generated temporaries or named locals. The MIR function header declares all locals with their types upfront. Type inference variables use `TyVid`, a plain index into the inference context. Polymorphic type variables in already-inferred types use `BoundVar` with de Bruijn levels. The two representations coexist because they serve different purposes: `TyVid` is for inference (mutable, open), `BoundVar` is for polymorphic types (immutable, closed). SA's design uses binding IDs throughout — there is no inference variable stage — but the staged lowering from named bindings to indexed slots follows the same HIR-to-MIR pattern. The MIR documentation is at https://rustc-dev-guide.rust-lang.org/mir/index.html.

### OCaml

The OCaml compiler (https://github.com/ocaml/ocaml) represents identifiers as `Ident.t = { name: string; stamp: int; scope: scope }`. The `stamp` is a globally unique integer incremented at each new binding. Equality is stamp equality; the name is for display. This is the simplest possible implementation of the binding ID approach and the clearest proof that it is sufficient for a production compiler. The ML type system that SA's module system draws from was developed in this tradition.

### Scala 3

Scala 3 / Dotty (https://github.com/scala/scala3) represents all named entities as `Symbol` with a unique ID. Variable and type references are `TermRef(prefix, symbol)` or `TypeRef(prefix, symbol)` — a direct reference to the binding site rather than a string lookup. The `prefix` captures the enclosing scope. This is structurally equivalent to a binding ID paired with a scope path, and is the direction the typed AST moves toward when the scope of a binding is explicitly part of its type, as CMTT requires.