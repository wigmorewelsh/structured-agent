# Runtime Extraction and Macro Library

## Original Goal

The stated goal was to move towards a standard library for the SA language. Native functions and modules are implemented in Rust and exposed to the language through a provider mechanism. For that to be possible from a separate crate, the runtime types — `ExpressionValue`, `AgentHandle`, `NativeFunction`, and related definitions — needed to live in a library crate that the standard library and macro crates could depend on without pulling in the compiler, MCP client, Gemini integration, or any other binary-level concern.

The work proceeded in three stages: extract the runtime into its own crate, create a proc-macro crate that reduces the boilerplate of writing native functions, and leave the system in a state where a standard library crate can be added with no further architectural changes.

## What Was Done

### structured-agent-runtime

A new library crate was created at `src/structured-agent-runtime/`. It contains everything a native function author needs and nothing from the execution pipeline.

The crate is organised into four modules:

- `expression.rs` — `ExpressionValue`, `ExpressionResult`, `ExpressionParameter`. These are the Arrow-backed value types that flow through the interpreter.
- `actor.rs` — `AgentHandle`, `AgentId`, `AgentMessage`, `AgentMessageContent`. These are the actor system types a native function uses to publish output or read incoming messages.
- `error.rs` — `RuntimeError` and `AgentError`. Both were previously defined inside the binary crate; moving them here allows stdlib functions to produce typed errors without depending on the binary.
- `types.rs` — `Type`, `Parameter`, `ExternalFunctionDefinition`, the `NativeFunction` trait, and the new `Module` trait.

The `Module` trait is the registration interface for grouped native functions:

```rust
pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn functions(&self) -> Vec<Arc<dyn NativeFunction>>;
}
```

The `Agent` struct remained in the binary because it holds `Arc<Runtime>` and calls `runtime.run_with_handle`. Moving it would have required moving the entire execution engine. To allow `Agent::new` to construct an `AgentHandle` without accessing its private fields, a `pair()` factory was added to `AgentHandle`:

```rust
pub fn pair() -> (AgentHandle, mpsc::UnboundedSender<...>) { ... }
```

The binary's `runtime/actor.rs` now re-exports the moved types and keeps only the `Agent` struct. The binary's `runtime/types.rs` was replaced with a single re-export line. The `src/types.rs` file in the binary retains `Span`, `SourceFiles`, `Spanned`, `Function`, `ExecutableFunction`, `FunctionProvider`, `LanguageEngine`, and `PrintEngine`, and re-exports `Type`, `Parameter`, `ExternalFunctionDefinition`, and `NativeFunction` from the runtime crate. All existing code in the binary continued to use the same module paths without change.

See [`src/structured-agent-runtime/src/`](../../src/structured-agent-runtime/src/) and the updated [`src/structured-agent/src/runtime/mod.rs`](../../src/structured-agent/src/runtime/mod.rs).

### structured-agent-macros

A proc-macro crate was created at `src/structured-agent-macros/`. It exposes two attribute macros.

`#[sa_fn]` transforms an async function into a `NativeFunction` implementation. Given:

```rust
/// Greet someone
#[sa_fn]
async fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
```

it generates a `GreetFunction` struct with the correct `parameters` and `return_type` metadata, a `Default` and `new()` impl, and a `NativeFunction` impl whose `execute` method validates the argument count, extracts each argument from `Vec<ExpressionValue>` by type, runs the original function body as a block expression, and converts the result back to `ExpressionValue`. Doc comments become the return value of `documentation()`. The `agent` parameter is always in scope in the function body.

`#[sa_module]` transforms an inline `mod` block containing `#[sa_fn]` functions into a module with all the generated structs plus a unit struct implementing `Module`:

```rust
#[sa_module]
mod math {
    #[sa_fn]
    async fn add(a: i64, b: i64) -> i64 { a + b }
}
// generates math::MathModule which implements Module
```

The crate is split into five modules to keep each concern small and to give clear extension points as more types are added:

- `types.rs` — `path_ident`, `generic_arg`, `is_unit_type`, `map_type_to_runtime`
- `extract.rs` — `extract_arg`, `extract_value`, `extract_option`
- `convert.rs` — `convert_result`, `to_expr_value`, `option_to_expr_value`
- `fn_gen.rs` — `generate_native_function` and its helpers
- `module_gen.rs` — `generate_module` and `partition_items`

`lib.rs` contains only the two proc-macro entry points, each of which calls `.unwrap_or_else(|e| e.to_compile_error())` so that errors are reported as compiler diagnostics rather than panics.

Type mapping, argument extraction, and result conversion are all recursive. `Option<Option<T>>` and similar nested types work without any special casing; each layer recurses through the same function. Adding a new scalar type requires one new arm in each of `map_type_to_runtime`, `extract_value`, and `to_expr_value`.

Supported types at time of writing: `String`, `bool`, `i64`, `()`, `Option<T>`, and `Vec<T>` (type mapping only; extraction of `Vec` is not yet implemented because it requires Arrow `ListArray` handling that is non-trivial to generate).

See [`src/structured-agent-macros/src/`](../../src/structured-agent-macros/src/) and the integration tests at [`src/structured-agent-macros/tests/`](../../src/structured-agent-macros/tests/).

### Dependency Graph

```
structured-agent-runtime
        ^           ^
        |           |
structured-agent-macros   structured-agent (binary)
        ^
        |
  (future stdlib)
```

The binary depends on the runtime. The macros crate depends on the runtime. A future stdlib crate will depend on both.

## What Is Not Yet Done

The functions in `src/structured-agent/src/functions/` — `PrintFunction`, `InputFunction`, `ReceiveFunction`, `TryReceiveFunction`, and the unstable list and option helpers — are the intended first content of a stdlib crate. They have not been moved. They remain in the binary and are registered there. Moving them requires creating a `structured-agent-stdlib` crate, rewriting each function using `#[sa_fn]` or `#[sa_module]`, and wiring the resulting `Module` implementations into `RuntimeBuilder`.

`NativeFunctionProvider` in the binary does not yet have a method to register a `Module`. The `RuntimeBuilder` does not yet have `with_module`. These are small additions but have not been made.

`Vec<T>` argument extraction is absent. The type maps correctly to `Type::list(T)` for metadata purposes, so the type checker and function registration work, but `execute` cannot extract a list argument. This means `HeadFunction` and `TailFunction` cannot be rewritten using the macro until this is added.

## Improvements That Should Be Made

### Span Hygiene

The internal variable names emitted by the macro — `__sa_result`, `__opt`, `__iv` — are generated with `Span::call_site()`, which is the default in `quote!`. The correct span for generated bindings that are not meant to be visible to the user is `Span::mixed_site()`. Without this, a user who happens to name something `__sa_result` inside their function body will get confusing behaviour rather than a clear error. The fix is to construct those identifiers explicitly:

```rust
let result_ident = Ident::new("__sa_result", Span::mixed_site());
```

This applies to `__sa_result` in `fn_gen.rs` and `__opt` and `__iv` in `extract.rs` and `convert.rs`.

### Input Validation

The `#[sa_fn]` macro does not validate its input at the entry point. Applied to a non-async function or a method with a `self` receiver, it will produce broken generated code rather than a clear diagnostic. Both should be checked before any code generation runs:

```rust
if input.sig.asyncness.is_none() {
    return Err(syn::Error::new_spanned(&input.sig, "sa_fn requires an async function"));
}
```

Similarly, any `FnArg::Receiver` in the inputs should be rejected with a span pointing at the receiver token.

### Attribute Argument Rejection

Both `sa_fn` and `sa_module` silently ignore any arguments passed to them. `#[sa_fn(nonsense)]` does nothing visible. The convention for macros that take no arguments is to parse the attribute stream as `syn::parse::Nothing`, which produces a proper error if anything is present.

### Error Accumulation

The first `?` in code generation aborts the entire macro and reports one error. When a function has two parameters of unsupported types, the user sees only the first failure. `syn::Error::combine` allows collecting independent errors and emitting them together. This is most valuable in `build_param_constructions` and `build_arg_extractions`, where each parameter is independent.

### Visibility Forwarding

The generated struct is always `pub` regardless of the visibility of the original function. The `ItemFn` carries a `vis` field that should be used instead. This matters when `#[sa_fn]` is used outside a module context or when a private helper function should not become part of the public API.

### Pass-Through Attributes

Attributes other than `#[doc]` on the original function are dropped. `#[cfg(...)]` and `#[cfg_attr(...)]` in particular should be forwarded to the generated struct. Without this, conditional compilation of native functions does not work.

### Crate Name Resolution

The generated code hardcodes `::structured_agent_runtime::` as the path prefix. If a consumer renames the crate in their `Cargo.toml` via `package = "..."`, the generated code will fail to compile. The [`proc-macro-crate`](https://crates.io/crates/proc-macro-crate) crate resolves the actual name of a dependency at macro expansion time and is the standard solution to this problem. This is not a concern while everything lives in one workspace but becomes relevant if the crates are ever published.

## See Also

- [`src/structured-agent-runtime/`](../../src/structured-agent-runtime/)
- [`src/structured-agent-macros/`](../../src/structured-agent-macros/)
- [`src/structured-agent/src/functions/`](../../src/structured-agent/src/functions/)
- [`src/structured-agent/src/runtime/native_provider.rs`](../../src/structured-agent/src/runtime/native_provider.rs)