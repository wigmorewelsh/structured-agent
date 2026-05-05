# Actor Refactor: Spawn Expression, Unified Call Path

## Original Goal

The previous handover ([0001](0001-actor-spawn-and-indexed-types.md)) identified that `FunctionKind::Spawn` and `FunctionKind::Actor` were a pretence — a signal on `Expression::Call` that certain calls were special — and that `elaborate_actor_method_call` was a stripped-down copy of `elaborate_call` that lacked type parameter unification, trait impl injection, and module param routing. The goal of this session was to remove that pretence:

- `spawn<Module>("key")` should be a dedicated `Expression::Spawn` node in both the untyped and typed AST, with `spawn` reserved as a keyword in the parser.
- `ActorRef<X>` method calls should go through exactly the same elaboration and compilation path as all other calls. The only distinction is that the elaborated `Call` node carries `target: Some(actor_ref_expr)` instead of prepending the receiver to the argument list.
- `Instruction::Spawn` should carry the module path statically rather than reading it from a runtime slot.
- The actor loop should have no special module-param injection logic. Args are args.

## What Was Done

### Parser and Untyped AST

`spawn` is now a reserved keyword. The parser produces `Expression::Spawn { type_arg: Type, key: Box<Expression>, span }` directly, rather than a generic `Call` node with function name `"spawn"`. The `parse_spawn` function is inserted before `parse_call` in the primary expression choice so that `spawn<X>(...)` is never misread as a path call. See [structured-agent-parser/src/lib.rs](../../src/structured-agent-parser/src/lib.rs) and [structured-agent-ast/src/ast/mod.rs](../../src/structured-agent-ast/src/ast/mod.rs).

### Typed AST

`Expression::Spawn { key: Box<Expression>, ty: Type, span }` was added to the typed AST. `Expression::Call` gained `target: Option<Box<Expression>>`. Every existing `Call` construction outside the actor path carries `target: None`. `FunctionKind::Spawn` and `FunctionKind::Actor` were removed from `FunctionKind`; only `Bytecode` and `External` remain. See [structured-agent-typed-ast/src/lib.rs](../../src/structured-agent-typed-ast/src/lib.rs) and [structured-agent-runtime/src/symbols.rs](../../src/structured-agent-runtime/src/symbols.rs).

### IL and Bytecode Compiler

`Instruction::Spawn` changed from `{ module_slot: Slot, key_slot: Slot, dest: Slot }` to `{ module_path: DefinitionPath, key_slot: Slot, dest: Slot }`. The module is now statically embedded in the instruction. `compile_spawn_expression` extracts the module path from the `ActorRef<T>` type in the typed AST and emits this instruction directly. `compile_call_expression` checks `target`: if `Some`, it compiles the target to a slot and emits `Instruction::CallActor`; otherwise it dispatches on `FunctionKind` as before. See [structured-agent-bytecode-compiler/src/compiler.rs](../../src/structured-agent-bytecode-compiler/src/compiler.rs) and [structured-agent-il/src/instruction.rs](../../src/structured-agent-il/src/instruction.rs).

### VM and Runtime

`execute_spawn` reads the key from a slot and the module path from the instruction. It no longer reads a `Module` value from a slot or extracts `module_params` from it. `spawn_actor` on the `RuntimeService` trait and its implementation in `engine.rs` no longer accept `module_params`. `actor_loop` lost the loop that pre-populated frame slots from `module_params`; the frame is now filled from `msg.args` starting at slot 1, identically to any other function call. See [structured-agent-vm/src/vm.rs](../../src/structured-agent-vm/src/vm.rs), [structured-agent/src/runtime/actor.rs](../../src/structured-agent/src/runtime/actor.rs), and [structured-agent/src/runtime/engine.rs](../../src/structured-agent/src/runtime/engine.rs).

### Type Checker — Synthesize

`synthesize_expression` has a `Spawn` branch that resolves the type argument and returns `RT::actor_ref(module_type)`. The old early return in `synthesize_call` that special-cased `function == "spawn"` was removed. See [structured-agent-typecheck/src/synthesize.rs](../../src/structured-agent-typecheck/src/synthesize.rs).

### Type Checker — Elaboration

`elaborate_method_call` was reduced to a thin resolver. It elaborates the receiver, then branches on whether the receiver type is an `ActorRef`:

- If `ActorRef<Module>`: extract the inner module path, build `DefinitionPath::for_function(module_path, method)`, look up the signature, set `target = Some(typed_receiver)` and `pre_typed_args = []`, then call `elaborate_call`.
- Otherwise: call `find_impl_fn` for the struct impl path, set `target = None` and `pre_typed_args = vec![typed_receiver]` (receiver as implicit first arg), then call `elaborate_call`.

`elaborate_call` was extended with three parameters: `pre_typed_args: Vec<typed_ast::Expression>`, `target: Option<Box<typed_ast::Expression>>`, and `pre_resolved: Option<(DefinitionPath, FunctionSignature)>`. When `pre_resolved` is `Some`, function resolution via `resolve_function_call` and routing via `resolve_call_routing` are both skipped. `pre_typed_args` are prepended to `all_args` ahead of the normally-elaborated arguments. The call site in `elaborate_expression` for regular `Expression::Call` nodes passes `vec![], None, None` for these parameters.

`elaborate_spawn` calls `build_module_instantiation` only to obtain the module's `DefinitionPath` for the `ActorRef` type annotation. It produces `typed_ast::Expression::Spawn { key, ty, span }` with no module params. See [structured-agent-typecheck/src/elaboration.rs](../../src/structured-agent-typecheck/src/elaboration.rs).

### Analysis Crates

All three analysis passes in `structured-agent-analysis` (unused expressions, return values, variables) and `structured-agent-il-analysis` were updated to handle the new `Spawn` variants. See [structured-agent-analysis/src/analysis/](../../src/structured-agent-analysis/src/analysis/) and [structured-agent-il-analysis/src/lib.rs](../../src/structured-agent-il-analysis/src/lib.rs).

## What Is Not Working

### Tests Not Verified

`cargo check` passes. The test suite was not run to completion during this session. The tests should be run before treating this work as done. The integration tests most likely to be affected are in [structured-agent/tests/integration/actor_test.rs](../../src/structured-agent/tests/integration/actor_test.rs).

### Generic Receiver Dispatch Dropped

The old `elaborate_method_call` handled `RT::Generic` receivers — that is, calls on a value whose type is a type parameter bounded by a trait, dispatched via `MethodBinding::Late`. That path called `find_implicit_param`, looked up the trait, and produced a late-bound call. The new `elaborate_method_call` only handles `ActorRef` and `Named` receivers. If the receiver type is `Generic`, the function returns `None` because neither the actor branch nor the `find_impl_fn` branch matches. Any code that calls a method on a generic-typed value via a trait bound will silently fail to elaborate. This needs to be reinstated. The logic to recover from the old function is in `trait_method_return_type` (see below) and in the deleted `elaborate_actor_method_call` code removed by the user mid-session.

### `actor_id_reflects_spawn_key` Rewritten

This test originally used `mod Counter(a: actor)` to inject the `actor` module as a module parameter, then called `actor_id()` through that parameter. Since module params are no longer injected by the actor loop, that pattern cannot work. The test was rewritten to use `use actor::actor_id` directly inside `Counter` without a module parameter. Whether the rewritten test passes against the new compilation path is unconfirmed.

## Dead Code and Things to Clean Up

`trait_method_return_type` in [elaboration.rs](../../src/structured-agent-typecheck/src/elaboration.rs) is now unreachable. It was only called from the generic receiver dispatch path that was lost when `elaborate_method_call` was restructured. It should be deleted when the generic receiver path is reinstated, or kept as reference while that work is in progress.

`build_module_instantiation` is imported and called in `elaborate_spawn` but only to resolve the `DefinitionPath` of the spawned module. The `params` on the returned `ModuleInstantiation` are discarded. A direct type resolution would be cleaner and the dependency on `build_module_instantiation` inside `elaborate_spawn` could be removed.

`routing` is set to `None` for all pre-resolved calls, which means actor method calls and struct method calls both bypass `resolve_call_routing`. For struct impl calls this is probably fine because the receiver is already the explicit first argument. For actor calls it is correct because actor dispatch happens via `CallActor`, not via module param routing. However, the skip is implicit and should be documented as intentional rather than discovered.

The `pre_resolved` parameter on `elaborate_call` causes the function to take eleven parameters. Extracting the resolution step into a dedicated type or struct would reduce the surface.

## Sources

- [structured-agent-ast/src/ast/mod.rs](../../src/structured-agent-ast/src/ast/mod.rs)
- [structured-agent-parser/src/lib.rs](../../src/structured-agent-parser/src/lib.rs)
- [structured-agent-typed-ast/src/lib.rs](../../src/structured-agent-typed-ast/src/lib.rs)
- [structured-agent-runtime/src/symbols.rs](../../src/structured-agent-runtime/src/symbols.rs)
- [structured-agent-il/src/instruction.rs](../../src/structured-agent-il/src/instruction.rs)
- [structured-agent-typecheck/src/synthesize.rs](../../src/structured-agent-typecheck/src/synthesize.rs)
- [structured-agent-typecheck/src/elaboration.rs](../../src/structured-agent-typecheck/src/elaboration.rs)
- [structured-agent-bytecode-compiler/src/compiler.rs](../../src/structured-agent-bytecode-compiler/src/compiler.rs)
- [structured-agent-vm/src/vm.rs](../../src/structured-agent-vm/src/vm.rs)
- [structured-agent/src/runtime/actor.rs](../../src/structured-agent/src/runtime/actor.rs)
- [structured-agent/src/runtime/engine.rs](../../src/structured-agent/src/runtime/engine.rs)
- [structured-agent/tests/integration/actor_test.rs](../../src/structured-agent/tests/integration/actor_test.rs)
- Previous handover: [0001-actor-spawn-and-indexed-types.md](0001-actor-spawn-and-indexed-types.md)
