# Actor Spawn and Yield

The actor system adds three things to SA's execution model: stable identity, message-passing dispatch, and cooperative suspension. This document covers the runtime design for the first two stages of that work — spawning actors and calling into them (Stage 1), and the cooperative `yield` mechanism that allows long-running background functions to interleave with direct calls (Stage 2).

The design is grounded in the virtual actor model from Orleans (Microsoft Research, "Orleans: Distributed Virtual Actors for Programmability and Scalability", 2014), adapted to SA's existing `Context`-threaded, bytecode-driven execution model. It introduces no new concurrency primitives beyond what Tokio already provides.

## How It Works

An actor is a Tokio task that owns a persistent `Context` and processes one message at a time. A `spawn<Module>("key")` expression in SA source looks up or creates that task via a global registry on `RuntimeService` and returns an `ActorRef`. A method call on an `ActorRef` sends an `ActorMessage` down a channel and awaits a `oneshot` reply — the same pattern used today by `publish_input_request` in `AgentHandle` ([actor.rs](../src/structured-agent-runtime/src/actor.rs)).

The actor task itself is a thin loop over a `mpsc` receiver. It receives a message, drives the VM against its persistent context, and sends the result back. The VM infrastructure — the explicit `CallFrame` stack, owned `VMState`, and async dispatch loop — already supports this without structural change.

Stage 2 extends the VM's return type so that hitting `ActorYield` does not end the function but parks the frozen `VMState` in the actor's pending queue. The actor task then processes the next mailbox message before resuming the parked function. This gives the cooperative, FIFO-ordered interleaving described in [actor-system.md](actor-system.md).

## Parser and Type System

### Surface Syntax

Actor method calls use dot notation — `actor.method(args)` — rather than the `::` path syntax used for module calls. This distinction is intentional: it signals at the call site that the receiver is a live actor instance, not a static module path, and that the call crosses a task boundary. The parser already handles this. `expr.name(args)` is parsed by the `.` postfix suffix loop in `parse_simple_expression` and produces `Expression::MethodCall { receiver, method, args }` with no changes required.

`spawn<Module>("key")` requires no dedicated parse rule. It is valid syntax for a generic function call — the same form used for any function with a type parameter — and parses as a `Call` node with function name `"spawn"` and a type argument of `"Module"`. No new AST node or keyword reservation is needed.

### Type Checker

**Elaborating `spawn`.** The type checker recognises a `Call` to the built-in function `"spawn"` by named. It resolves the type argument against the current module's scope, verifies the named module exists, and returns `Type::Parameterized("ActorRef", [Type::Named(module_path)])`. Using `Parameterized` is consistent with how `List<T>` and `Option<T>` are represented; no new `Type` variant is needed. A helper `Type::is_actor_ref()` alongside the existing `is_list()` and `is_option()` methods makes pattern-matching ergonomic. The elaborated typed AST node is an ordinary `Call` node carrying the resolved `ActorRef<T>` type and a new `FunctionKind::Spawn` so the compiler can emit the correct instruction.

**Elaborating `actor.method(args)`.** This runs through the existing `elaborate_method_call` path, which already handles dispatch by receiver type. A new branch at the top checks whether the receiver's type satisfies `is_actor_ref()`. If so, the inner type parameter is extracted, the method is looked up on that module's definition (the same resolution used for a normal bytecode call), argument types are checked, and the call is produced with a new `FunctionKind` variant:

```
pub enum FunctionKind {
    Bytecode,
    External,
    Actor,
}
```

The typed `Call` node carries `MethodBinding::Early(fn_path)` — the function is statically known from the module type — and `FunctionKind::Actor`. The elaborated receiver is prepended as the first argument, as the existing method call elaboration does today. No changes to `MethodBinding` or the `Call` node structure are needed. The full set of new `FunctionKind` variants is therefore:

```
pub enum FunctionKind {
    Bytecode,
    External,
    Spawn,
    Actor,
}
```

### Compiler

A `Call` with `FunctionKind::Spawn` compiles by loading the module reference into a slot, compiling the key expression into a slot, and emitting `Instruction::Spawn`. A `Call` with `FunctionKind::Actor` compiles by reading the actor ref from the first argument slot (the receiver) and emitting `Instruction::CallActor`. Both sit alongside the existing `Bytecode`, `External`, and `CallIndirect` branches in `compile_call_expression` and require no restructuring of the surrounding logic.

## Message Dispatch (Stage 1)

### ActorMessage and ActorRef

An `ActorMessage` carries a function path, the evaluated arguments, and a `oneshot` sender for the reply:

```rust
pub struct ActorMessage {
    pub function_name: DefinitionPath,
    pub args: Vec<ExpressionResult>,
    pub reply: oneshot::Sender<Result<ExpressionResult, String>>,
}
```

This is structurally identical to `RequestUserInput` in `AgentMessageContent`, which uses the same `oneshot` reply pattern to bridge an async request across Tokio task boundaries.

`ActorRef` implements `RuntimeValue` and carries the mailbox sender:

```rust
pub struct ActorRef {
    pub module_path: DefinitionPath,
    pub actor_id: String,
    mailbox: mpsc::Sender<ActorMessage>,
}
```

Because `ExpressionValue::Dynamic(Arc<dyn RuntimeValue>)` already exists, `ActorRef` requires no new variant on `ExpressionValue`. It is cloneable and `Send + Sync`.

### The Actor Registry

The registry maps string keys to mailbox senders and lives on `RuntimeService`:

```rust
fn actor_registry(&self) -> Arc<ActorRegistry>;
```

`ActorRegistry` is an `Arc<Mutex<HashMap<String, mpsc::Sender<ActorMessage>>>>`. Making it part of `RuntimeService` gives every VM instance access via `context.runtime().actor_registry()`, which matches the Orleans model of a global, location-transparent registry. The registry holds only the sender side; the actor task owns the receiver.

### New IL Instructions

Two instructions are added to the IL:

```
Spawn    { module_slot: Slot, key_slot: Slot, dest: Slot }
CallActor { actor_slot: Slot, fn_name: DefinitionPath, params: Vec<Slot>, dest: Slot }
```

`Spawn` reads an `ExpressionValue::Module` from `module_slot` and a string key from `key_slot`. If the registry contains an entry for that key it returns a clone of the sender wrapped in an `ActorRef`; otherwise it spawns a new actor task, registers the sender, and returns the ref. The returned `ActorRef` is written to `dest` as an `ExpressionValue::Dynamic`.

`CallActor` is a peer of `CallIndirect`, not an extension of it. The type checker knows at the call site whether the target is a module reference or an actor ref, and the compiler emits the appropriate instruction. This avoids a runtime branch in `CallIndirect` and keeps the two dispatch paths distinct. `CallActor` in the VM reads the `ActorRef` from `actor_slot`, constructs an `ActorMessage` with a fresh `oneshot` pair, sends it, and `.await`s the reply. The call suspends the calling Tokio task but does not block the thread.

### The Actor Task Loop

Each actor runs a Tokio task of the form:

```rust
async fn actor_loop(
    mut mailbox: mpsc::Receiver<ActorMessage>,
    context: Context,
    runtime: Arc<dyn RuntimeService>,
) {
    let mut ctx = context;
    while let Some(msg) = mailbox.recv().await {
        let (next_ctx, result) = dispatch(&ctx, &runtime, msg.function_name, msg.args).await;
        ctx = next_ctx;
        let _ = msg.reply.send(result);
    }
}
```

`dispatch` constructs a `BytecodeFunctionExpr` for the named function and calls `.execute(ctx, args)`. The returned `Context` becomes the actor's new persistent context, accumulating any events pushed directly in the top-level function body. Sub-calls within the function create child contexts via the existing `create_child` / `restore_parent` mechanism and do not modify the actor's root.

### Context Threading

The actor's persistent `Context` serves as the root for all LLM calls made during the actor's lifetime. When a message is dispatched, the function executes directly in this root context; it is not wrapped in a child. Sub-functions called from within it use the normal child-and-restore scoping. The LLM, when invoked at any depth, walks the full chain via `iter_all_context_events()` and sees the accumulated history of all prior calls. After dispatch, the returned context — carrying any new top-level events — replaces the actor's previous root.

This matches the design intent in [actor-system.md](actor-system.md): the actor context grows across calls, while child scopes are still popped normally on return.

```
Caller VM                 Actor Task
─────────                 ──────────
CallActor ──ActorMessage──▶ recv()
  .await  ◀──reply.send()── VM::execute(actor_ctx, args)
                             → actor_ctx updated with new events
```

## Cooperative Yield (Stage 2)

### VMOutcome

The VM's `execute` method currently returns `Result<(Context, ExpressionResult), String>`. Stage 2 replaces this with an outcome type:

```rust
enum VMOutcome {
    Complete(Context, ExpressionResult),
    Yielded(VMState),
}
```

`VMState` — already a concrete, owned struct containing `call_stack: Vec<CallFrame>` and `context: Context` — is the natural representation of a suspended execution. No new state needs to be introduced; the existing stack is frozen at the yield point and moved into `Yielded`.

A new `ActorYield` instruction is added to the IL, distinct from the existing `Yield` which is reserved for durable execution. The compiler emits `ActorYield` for `yield` statements within actor function bodies. When the VM dispatch loop encounters `ActorYield` it returns `VMOutcome::Yielded(state)` immediately, without popping the call stack.

### The Pending Queue

The actor task gains a pending queue alongside its mailbox:

```rust
struct ActorTask {
    mailbox: mpsc::Receiver<ActorMessage>,
    pending: VecDeque<(VMState, oneshot::Sender<Result<ExpressionResult, String>>)>,
    context: Context,
}
```

The loop becomes:

1. Receive the next `ActorMessage` from the mailbox.
2. Run the VM. If `Complete`, send the reply immediately.
   If `Yielded`, push `(state, reply)` onto `pending`.
3. After each message — whether complete or yielded — tick the pending queue: resume each parked `VMState` once, advancing it to the next `Yielded` or `Complete`.

Because the actor is a single Tokio task, only one execution is in flight at any moment. The cooperative lock is implicit: a function holds it from the point it is dispatched until it yields or completes. No explicit mutex is needed within the actor.

```
Mailbox         Pending Queue
───────         ─────────────
msg A ──▶ run ──▶ Yielded ──▶ [ (stateA, replyA) ]
msg B ──▶ run ──▶ Complete ──▶ send replyB
          tick pending ──▶ resume stateA ──▶ Yielded ──▶ [ (stateA', replyA) ]
msg C ──▶ run ──▶ Complete ──▶ send replyC
          tick pending ──▶ resume stateA' ──▶ Complete ──▶ send replyA
```

### Scheduling Guarantees

The FIFO ordering described in [actor-system.md](actor-system.md) follows from the queue structure. New messages are appended to the mailbox and processed before parked continuations are ticked, which means a newly arrived call sees the context as it stood when the background function last yielded. Parked functions resume in the order they were suspended. The context accumulated by a yielded function between suspension and resumption is therefore deterministic for a given call sequence.

## Relationship to Existing Patterns

The `oneshot` reply in `ActorMessage` mirrors `publish_input_request` exactly ([actor.rs L86–95](../src/structured-agent-runtime/src/actor.rs)). The actor task loop mirrors the `Agent::run` pattern: a long-lived Tokio task processing messages from a channel. `VMState` as a suspendable unit mirrors the way `BytecodeFunctionExpr::execute` already owns and threads `Context` through the VM as a moved value — the only addition is moving that state out of the loop rather than completing it.

The two new IL instructions follow the existing convention: `CallBytecode`, `CallExternal`, `CallIndirect`, and `CallNative` are already four distinct call variants covering different dispatch mechanisms. `Spawn` and `CallActor` add two more without altering the existing paths.

## Open Questions

Cancellation of parked continuations — what happens when the actor task is dropped while functions are suspended in the pending queue — is not addressed here. The reply senders will be dropped, causing the callers' `oneshot` receivers to observe a closed channel error. Whether that is surfaced as a runtime error or silently discarded depends on how the calling VM handles `CallActor` failures, which is an open design point.

Whether `spawn` and `yield` should be blocked from use as identifiers — by a reserved word check in `identifier_raw` or by parse ordering — is a minor open question. All other keywords are enforced by ordering; consistency argues for the same approach here, though `spawn` as a generic call means it would only be shadowed in practice if a user defined a function of the same name.

## See Also

- [actor-system.md](actor-system.md) — language-level design for the actor system
- [module-system.md](module-system.md) — SA module system including signatures and dependency injection
- [durable-execution.md](durable-execution.md) — the separate durable `Yield` instruction and execution persistence
- [structured-agent-runtime/src/actor.rs](../src/structured-agent-runtime/src/actor.rs) — `AgentHandle`, `AgentMessage`, `publish_input_request`
- [structured-agent-vm/src/vm.rs](../src/structured-agent-vm/src/vm.rs) — VM dispatch loop, `VMState`, `CallFrame`
- Microsoft Research. (2014). "Orleans: Distributed Virtual Actors for Programmability and Scalability." https://www.microsoft.com/en-us/research/publication/orleans-distributed-virtual-actors-for-programmability-and-scalability/
- Hewitt, C., Bishop, P., Steiger, R. (1973). "A Universal Modular ACTOR Formalism for Artificial Intelligence." IJCAI 1973.
