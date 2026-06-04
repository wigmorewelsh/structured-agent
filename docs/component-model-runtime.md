# Component Model Runtime

SA's runtime is designed to run on top of the WebAssembly Component Model, using Wasmtime as the host. The SA runtime — actors, context threading, durable execution, MCP/ACP dispatch — remains implemented in Rust and Tokio. SA modules compile to WebAssembly components, and the runtime calls into them using Wasmtime's component embedding API. This document describes how SA's execution model maps onto Component Model Preview 3, which shipped in February 2026, and what is available in Wasmtime today.

For the actor design this document builds on, see [actor-spawn-and-yield.md](actor-spawn-and-yield.md) and [actor-system.md](actor-system.md). For durable execution and the `Snapshot` instruction, see [durable-execution.md](durable-execution.md).

## Architecture

The SA runtime is the host. SA modules are guests. The component model defines the boundary between them. Nothing in the runtime — the actor registry, the context chain, the mailbox, the pending queue — moves into WebAssembly. What moves into WebAssembly is the compiled SA module code: the bytecode functions that call LLM primitives, manipulate context, and call other modules.

```
┌─────────────────────────────────────────┐
│  SA Runtime (Rust / Tokio)              │
│  actor_loop · context chain · registry  │
│  MCP / ACP / LLM dispatch               │
├─────────────────────────────────────────┤
│  Wasmtime  (component-model-async)      │
│  call_concurrent · run_concurrent       │
├──────────────┬──────────────────────────┤
│  Actor A     │  Actor B                 │
│  WASM        │  WASM                    │
│  component   │  component               │
└──────────────┴──────────────────────────┘
```

Each actor is a separate Wasmtime component instance. The `actor_loop` function in Rust owns the mailbox, the pending queue, and all scheduling decisions. It calls into the component using `Func::call_concurrent` and drives the event loop with `StoreContextMut::run_concurrent`. When the guest hits `thread.yield`, `run_concurrent` returns control to the Rust loop, which then decides whether to process the next mailbox message or tick the pending queue — exactly as it does today against the SA bytecode VM.

## Component Model Preview 3 Concurrency

Preview 3, specified in the Component Model Concurrency explainer, adds first-class async to the component model. The relevant primitives are as follows.

A component export declared `async` in WIT may block before returning. Each call to such an export creates a new green thread within the component instance. These threads are cooperative: they switch only at explicit points, not preemptively. The spec defines this in `Concurrency.md`: "Until the Core WebAssembly shared-everything-threads proposal allows Core WebAssembly function types to be annotated with `shared`, all threads must execute cooperatively in a sequentially-interleaved fashion."

The `thread.yield` built-in suspends the current thread and allows the host scheduler to resume it later. From the Canonical ABI:

```python
def canon_thread_yield(cancellable):
    thread = current_thread()
    cancelled = thread.yield_(cancellable)
    return [cancelled]
```

If called outside an `async`-typed function it is a no-op rather than a trap, which means SA's `ActorYield` instruction compiles to `thread.yield` unconditionally without needing to track async context.

The `task.return` built-in allows a task to pass its return value to the caller and then continue executing. This is the mechanism for long-running background tasks — the start function pattern described in `Concurrency.md`: "if a component-level start function is lifted using the async ABI, it may block after calling `task.return`, and may thus serve as a long-running background task to which work can be dispatched." This is the component model analogue of `defer actor.init()`.

A `ComponentInstance` holds an `exclusive: Optional[Task]` field. Sync-ABI and stackless-async (callback) tasks acquire this component-wide lock on entry and release it at each yield or return. Only one such task runs at a time within the component. This implicit serialisation is the component model's equivalent of the Tokio actor task: reentrancy is prevented structurally rather than by an explicit mutex.

Sources: [WebAssembly/component-model `design/mvp/Concurrency.md`](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md) and [`design/mvp/CanonicalABI.md`](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md).

## The Actor Loop

The `actor_loop` in [`src/structured-agent/src/runtime/actor.rs`](../src/structured-agent/src/runtime/actor.rs) is unchanged in its structure. Its current implementation drives the SA bytecode VM via `vm.execute_outcome` and `vm.resume_outcome`. In the WASM target these calls are replaced by Wasmtime's `call_concurrent` and `run_concurrent`, but the mailbox, pending queue, FIFO tick ordering, and context threading logic are all host-side and remain in Rust.

The key determinism property — that a new mailbox message is always processed before parked continuations are ticked — is preserved because the Rust loop controls when `run_concurrent` is called. The CM3 spec's nondeterminism note applies to the host scheduler; since SA is the host, SA defines the schedule. The actor loop's explicit tick-after-receive semantics are not disrupted.

The context chain is an immutable linked list. The actor's root context pointer advances as each LLM call appends a new node. This pointer lives in the Rust host, not in the WASM guest. When `do_work()` runs after `init()` has yielded, it inherits the updated root pointer because the Rust actor loop passes the current context into the component call — the same mechanism as today, just crossing a Wasmtime call boundary rather than a VM stack frame boundary.

The ordering between `init()` and `do_work()` in the example below is guaranteed by the deterministic-run-until-block property of CM3 async calls, not by a FIFO queue:

```sa
fn main(): () {
  let actor = spawn<SomeActor>("1")
  defer actor.init()
  let result = actor.do_work()
  return result
}
```

The CM3 spec states: "If a component `a` asynchronously calls the export of another component `b`, control flow deterministically transfers to `b` and then back to `a` when `b` returns or blocks." `defer actor.init()` issues the call; init runs to its first `thread.yield`; control returns to the caller; the caller then issues `actor.do_work()`. By the time `do_work` reaches the LLM call, the context chain already contains the node appended by `init`. The LLM sees both chunks of context in its window.

## Wasmtime Support (May 2026)

Wasmtime 44.0.1 implements the component model async ABI behind the `component-model-async` Cargo feature. The following are available today.

`Func::call_concurrent` starts a host-to-guest call against an async-lifted component export and returns a `JoinHandle`. `StoreContextMut::run_concurrent` drives the per-store event loop, returning when the guest blocks or completes. `StoreContextMut::spawn` and `Accessor::spawn` queue background host tasks into the same event loop. `LinkerInstance::func_wrap_concurrent` registers concurrent host import functions that receive an `Accessor` for store access between await points. `FutureReader`, `FutureProducer`, `StreamReader`, `StreamProducer`, `StreamConsumer`, and related types implement the CM3 `future<T>` and `stream<T>` primitives. `ErrorContext` is also implemented.

The internal implementation in `crates/wasmtime/src/runtime/component/concurrent.rs` uses fibers (`wasmtime-internal-fiber`), `FuturesUnordered`, and a `VecDeque`-based task queue — structurally the same pattern as SA's existing actor loop.

To enable:

```toml
wasmtime = { version = "44", features = ["component-model", "component-model-async", "async"] }
```

Source: [`wasmtime/crates/wasmtime/src/runtime/component/concurrent.rs`](https://github.com/bytecodealliance/wasmtime/blob/main/crates/wasmtime/src/runtime/component/concurrent.rs).

## Instruction Translation

SA's IL instructions fall into four groups when targeting WASM.

Standard control flow and value instructions — `Br`, `BrFalse`, `BrTrue`, `Switch`, `Ret`, `Mov`, `LdcBool`, `LdcInt`, `LdcUnit` — map directly to WASM opcodes. String and structural instructions — `LdcStr`, `StrConcat`, `ListCreate`, `StructNew`, `StructGet`, `MatchType` — use WASM GC types or host imports for heap allocation.

Instructions that invoke SA runtime services — `LlmPlaceholder`, `LlmSelect`, `LlmGenerate`, `CtxEvent`, `CtxChild`, `CtxRestore`, `Snapshot`, `Spawn`, `CallActor`, `CallExternal`, `LoadModule`, `CallVirtual`, `MetaFunction` — become WIT import calls. The guest calls them; the Rust host implements them via the Wasmtime linker. A representative WIT interface:

```wit
package sa:runtime;

interface llm {
    generate: async func(return-type: string) -> value;
}

interface context {
    event: func(val: value);
    child: func();
    restore: func();
}

interface actor {
    spawn: func(module-path: string, key: string) -> actor-ref;
    call: async func(ref: actor-ref, fn-name: string, params: list<value>) -> value;
}
```

`ActorYield` is the exception. It is not a WIT import. It compiles directly to the `thread.yield` canonical ABI built-in, which is wired into the component model itself rather than exposed as a function import.

`CallNative` requires no redesign. Native functions are Rust code that compiles to `wasm32-wasip2` and is linked into the component. The instruction compiles to a direct WASM `call`. Native functions that need runtime services (context, LLM) import them via the same WIT interface as any other guest code.

`CallBytecode` compiles to a direct WASM `call` within the same component, since the callee is SA source compiled to WASM alongside the caller.

## Calling Conventions and DefinitionPath

SA passes `DefinitionPath` values — not function references — as module parameters. This is intentional. `DefinitionPath` is a stable, serialisable string path that the runtime resolves at call time. Two runtime properties depend on this indirection.

Durable execution requires the call stack to be serialised and resumed after a process restart. A function reference cannot be serialised. A `DefinitionPath` can. The `Snapshot` instruction records paths, not addresses, precisely so that a resumed execution can locate the current version of a function after a cold start.

Hot module reloading requires that a call to a module method always reaches the currently loaded version. Resolving a function reference at `LoadModule` time and storing it in a GC struct would bake in the version at load time. Passing the path and resolving at dispatch time means the next call after a reload automatically reaches the new version.

The consequence for WASM is that `LoadModule` and `CallVirtual` remain host dispatch calls rather than intra-component calls through a function reference struct. `LoadModule` returns an opaque handle into the host-side module registry, keyed by `DefinitionPath`. `CallVirtual` calls the host dispatch import with that handle and a method name string. The host resolves the path, locates the component instance for the current module version, and invokes the export.

Where the module type is statically known at the call site — `MethodBinding::Early` in the type checker — the compiler may emit a direct WASM `call` and skip the dispatch import entirely. This optimisation is safe because an early-bound call site has no need for the runtime resolution that hot reload and durable execution require.

## See Also

- [actor-spawn-and-yield.md](actor-spawn-and-yield.md) — actor runtime design including `VMOutcome`, pending queue, and context threading
- [actor-system.md](actor-system.md) — language-level actor design
- [durable-execution.md](durable-execution.md) — `Snapshot`, durable `Yield`, and execution persistence
- [module-system.md](module-system.md) — SA module system, signatures, and dependency injection
- WebAssembly Component Model. "Concurrency Explainer." [`design/mvp/Concurrency.md`](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md)
- WebAssembly Component Model. "Canonical ABI Explainer." [`design/mvp/CanonicalABI.md`](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md)
- Bytecode Alliance. Wasmtime `concurrent.rs`. [`crates/wasmtime/src/runtime/component/concurrent.rs`](https://github.com/bytecodealliance/wasmtime/blob/main/crates/wasmtime/src/runtime/component/concurrent.rs)
- Microsoft Research. (2014). "Orleans: Distributed Virtual Actors for Programmability and Scalability." https://www.microsoft.com/en-us/research/publication/orleans-distributed-virtual-actors-for-programmability-and-scalability/
- Rossberg, A. (2015). "1ML — Core and Modules United." ICFP 2015.
