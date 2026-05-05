# Call Elaboration Refactor

## Original Goal

The previous session ([0002](0002-actor-refactor.md)) added actor support but left `elaborate_call` and `elaborate_method_call` producing `Expression::Call` through two divergent code paths. The method call path was missing generic type unification, placeholder argument support, implicit trait argument injection, and module routing. At runtime there is no distinction between a regular call and a method call — both compile to the same instruction with the same argument layout — so the two paths must produce equivalent output. The goal of this session was to align them, fix a latent argument ordering bug, and decompose the resulting shared logic into named, single-purpose functions.

## What Was Done

### Alignment of `elaborate_method_call`

The concrete-receiver path of `elaborate_method_call` previously returned `sig.return_type` verbatim and elaborated arguments with no unification, no placeholder handling, no type literal args, and no implicit trait args. The fix required changes in two places.

In `synthesize.rs`, the `MethodCall` branch for concrete receivers now creates a `Unifier`, unifies the receiver against the first parameter, skips placeholder arguments (matching the behaviour of `synthesize_call`), and returns `unifier.apply_subst(&sig.return_type)` rather than `sig.return_type` directly. Without this, the check pass would reject generic method calls before elaboration even ran.

In `elaboration.rs`, the concrete-receiver path was rewritten to delegate to the shared `build_typed_call` function described below.

Two new typed-AST tests cover these cases: `method_call_placeholder_carries_parameter_type` and `method_call_generic_return_type_resolves`. Both confirm that a method call on a concrete receiver now behaves identically to a top-level function call with the same signature. See [structured-agent-typecheck/src/tests.rs](../../src/structured-agent-typecheck/src/tests.rs).

### Argument Ordering Bug

`elaborate_function` (the declaration side) produces function parameters in the order: type params, module params, implicit trait params, user params. The previous call site assembled arguments in the order: type, trait, routing, user — placing implicit trait args before module routing args. The correct order is type, routing, trait, user.

The bug was silent because no test exercised a function with both module params and type-param trait bounds simultaneously. It is now fixed in `elaborate_arguments`. A TODO comment in `typed_ast_tests` notes that a test for this specific combination requires a multi-module setup that the single-module `check_typed` helper cannot express.

### `ResolvedCallee` and Callee Resolution

A private `ResolvedCallee` struct groups the three things known about a call target before argument building begins:

```rust
struct ResolvedCallee {
    sig: FunctionSignature,
    binding: typed_ast::MethodBinding,
    routing: Option<CallRouting>,
}
```

Two functions produce it. `resolve_callee_for_call` handles top-level function calls: it resolves the function name through the use-path system, fetches the signature, resolves routing, and derives the binding (Late if the function is accessed through a module param, Early otherwise). `resolve_callee_for_method` handles concrete struct method calls: it uses `find_impl_fn` to locate the impl function, fetches the signature, and resolves routing using the struct type name as the alias.

### Argument Building Decomposition

`build_typed_call` is a thin shell. It seeds a `Unifier` from explicit type args and the elaborated receiver, delegates all argument construction to `elaborate_arguments`, applies the substitution to the return type, and assembles `Expression::Call`.

`elaborate_arguments` enforces the execution order that the borrowing model requires. `elaborate_user_args` runs first and holds the sole `&mut Unifier` borrow; it elaborates each pending argument, unifies it against its declared parameter type, and handles placeholders by substituting the declared parameter type directly. Once that borrow is released, the three read-only functions run:

- `elaborate_type_args` — emits a `TypeLiteral` or `Variable` expression for each type parameter of a `Bytecode` function, resolving from explicit type args or the now-saturated unifier.
- `elaborate_routing_args` — converts each `CallModuleArg` into a `Variable` (for `FromParam`) or a `ModuleInstance` (for `Concrete`). No unifier involvement.
- `elaborate_implicit_trait_args` — for each type parameter with trait bounds whose concrete type is known to the unifier, looks up the solved impl via `impl_for_type_and_trait` and emits a `ModuleInstance`.

Assembly order: type args, routing args, trait args, receiver, user args.

The data flow through `build_typed_call` can be read as:

```
AST expression
  └── resolve_callee_for_{call,method}
        └── ResolvedCallee { sig, binding, routing }
              └── build_typed_call
                    ├── seed unifier (type_args + receiver)
                    └── elaborate_arguments
                          ├── elaborate_user_args       &mut Unifier  → user args
                          ├── elaborate_type_args        &Unifier     → type exprs
                          ├── elaborate_routing_args     (no unifier) → routing exprs
                          └── elaborate_implicit_trait_args &Unifier  → trait exprs
                    └── Expression::Call { binding, kind, arguments, ty, ... }
```

### `target: None`

The `target` field added to `Expression::Call` in [0002](0002-actor-refactor.md) was missing from three `Call` construction sites in `elaboration.rs`, causing a build failure. All three are now set to `None`. Actor call elaboration — where `target` would carry the actor reference expression — is not yet implemented; `Expression::MethodCall` on an `RT::actor_ref(...)` receiver silently returns `None` from `elaborate_method_call` (falls through the `_ => return None` arm on the receiver type match).

## What Is Not Working

### Generic Receiver Path

The `RT::Generic` branch of `elaborate_method_call` — the path taken when calling a trait method on a type-parameter-bound value inside a generic function body — does not go through `elaborate_arguments`. It constructs its argument list directly as `[typed_receiver] + elaborated_args`, bypassing `elaborate_user_args`, so placeholder arguments in that position are broken. It also emits no type literal args and no implicit trait args for any bounds on the method itself beyond the receiver's own trait.

More seriously, `find_implicit_param` selects the trait witness module by searching for any environment variable whose name starts with `"__T__"`. If the type parameter has multiple bounds (e.g., `T: Add + Mul`), this is a prefix match over a `HashMap` and the result is non-deterministic. The call could bind to the wrong trait's module. The fix requires identifying which trait defines the method being called, then selecting specifically that implicit param. See `find_implicit_param` at [structured-agent-typecheck/src/elaboration.rs](../../src/structured-agent-typecheck/src/elaboration.rs).

### No Test Coverage for Routing + Trait Bounds

The argument ordering fix (type → routing → trait → user) is not covered by any test because the single-module `check_typed` test helper cannot construct a program where a function has both module params and trait-bounded type params simultaneously. This combination is valid in the language and the fix is correct by inspection against `elaborate_function`'s parameter ordering, but it has no regression test.

### `synthesize_expression` Duplication

The `MethodCall` branch in `synthesize.rs` now contains a local unifier and a parameter-zip loop that duplicate the structure of `elaborate_user_args`. The two cannot share code without introducing a dependency from `synthesize` into `elaboration` or extracting both into a third module. This is noted as a known duplication.

### Actor Method Calls

Actor method calls (`receiver_type.is_actor_ref()`) return `None` from `elaborate_method_call` and `None` from `synthesize_expression`. The synthesis path in `synthesize.rs` does handle actor refs (it reads the module path from the actor ref type and returns the function's return type) but the elaboration path does not produce a typed `Expression::Call` node for them. The `target` field and `CallActor` instruction exist and the bytecode compiler handles them, but nothing produces a `Call { target: Some(...) }` node yet.
