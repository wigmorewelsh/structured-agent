# Stdlib Module Imports

SA programs that use standard library functions must currently declare each one with an `extern fn` statement in every source file that needs it. This is mechanical repetition: the type signature is already encoded in the `#[sa_fn]` macro output and available through the `NativeFunction` trait. This document describes the change that eliminates that repetition, allowing a program to write `use unstable::head` and have the type checker resolve the signature automatically.

## Current State

The type checker learns about native functions exclusively from `extern fn` declarations in SA source. The runtime learns about them through `RuntimeBuilder::with_module`. The two paths share no information. A program that calls `head` must include:

```/dev/null/example.sa#L1-2
pub extern fn head<T>(list: List<T>): Option<T>
use unstable::head
```

The `extern fn` line duplicates what the `#[sa_fn(type_params = "T")]` annotation on the Rust implementation already states. If the implementation changes its signature, the source declaration must be updated manually.

## Design

### Type Conversion Bridge

`NativeFunction` exposes its SA-level signature through `parameters()`, `return_type()`, and `type_params()`, all using `runtime::Type`. A conversion function in the compiler crate maps these to `ast::Type`:

```/dev/null/bridge.rs#L1-8
runtime::Type::String    → ast::Type::String
runtime::Type::Boolean   → ast::Type::Boolean
runtime::Type::Int       → ast::Type::Int
runtime::Type::Unit      → ast::Type::Unit
runtime::Type::List(t)   → ast::Type::List(convert(t))
runtime::Type::Option(t) → ast::Type::Option(convert(t))
runtime::Type::Struct(n) → ast::Type::Struct(n)
runtime::Type::Generic(n)→ ast::Type::Generic(n)
```

This conversion is total and requires no fallback. It lives in `compiler/mod.rs` or a small `compiler/native_bridge.rs`.

### `Compiler` gains a native module registry

`Compiler` is currently a stateless struct holding only a `CodespanParser`. It gains a `modules: HashMap<String, Arc<dyn Module>>` field, populated through a `with_module` builder method:

```/dev/null/compiler.rs#L1-5
pub struct Compiler {
    parser: CodespanParser,
    modules: HashMap<String, Arc<dyn Module>>,
}
```

`Compiler::with_module(module: Arc<dyn Module>) -> Self` inserts the module under the key returned by `Module::name()`.

### `collect_sigs` resolves native modules

When `collect_sigs` processes a `Definition::Use { path, alias, .. }` where `path` has at least two segments and the first segment matches a key in `Compiler::modules`, it synthesises an `ExternalSig` for the named function using the type conversion bridge. The entry is inserted into the `SigTable` exactly as a parsed `extern fn` declaration would be. No other part of the pipeline changes.

If the named function does not exist in the module, `collect_sigs` returns a `TypeError` naming the missing function and the module.

Wildcard imports (`use unstable::*`) are not supported in this change and are deferred to future work.

### `RuntimeBuilder` wires both sides

`RuntimeBuilder` already creates the `Compiler` inside its `build()` method via `Arc::new(Compiler::new())`. It also already holds a `NativeFunctionProvider` populated by `with_module`. To pass module information to the compiler, `RuntimeBuilder` gains a `modules: Vec<Arc<dyn Module>>` field. `with_module` is updated to push to this vec in addition to its existing `native_provider` population. In `build()`, the compiler is constructed as:

```/dev/null/engine.rs#L1-5
let compiler = self.modules.iter().fold(Compiler::new(), |c, m| {
    c.with_module(Arc::clone(m))
});
```

The CLI touches nothing. It already delegates compiler construction to `RuntimeBuilder::build()`.

## Files Changed

| File | Change |
|---|---|
| `structured-agent/src/compiler/mod.rs` | Add `modules` field and `with_module` builder to `Compiler`; add `runtime_type_to_ast` conversion |
| `structured-agent/src/compiler/sigs.rs` | Extend `collect_sigs` to synthesise `ExternalSig` from native module registry on `use` |
| `structured-agent/src/runtime/engine.rs` | Add `modules: Vec<Arc<dyn Module>>` to `RuntimeBuilder`; update `with_module` and `build()` |
| `structured-agent-stdlib/src/unstable/mod.rs` | No change — `name()` already returns `"unstable"` |

## Usage After the Change

A program that previously required:

```/dev/null/before.sa#L1-4
pub extern fn head<T>(list: List<T>): Option<T>
use unstable::head

fn main(xs: List<String>): Option<String> { return head(xs) }
```

becomes:

```/dev/null/after.sa#L1-3
use unstable::head

fn main(xs: List<String>): Option<String> { return head(xs) }
```

## End-to-End Tests

A small number of integration tests verify the full path from SA source through compilation and execution against the real stdlib runtime. These tests register `UnstableModule` with the `RuntimeBuilder`, compile SA source that uses `use unstable::head` (and the other unstable functions) without any `extern fn` declarations, and assert on the execution result. They complement the existing unit tests for type checking and bytecode emission, which remain in place.

## Future Work

Wildcard imports — `use unstable::*` — would add every function from a native module into scope without naming each one. This is deferred until there is a demonstrated need; explicit imports are preferable for clarity in small programs.

Once generic structs and sum types allow `Option<T>` and `List<T>` to move from compiler built-ins to the standard library, the native functions defined here would become ordinary SA functions. The import mechanism described in this document would remain in place for any native functions that continue to require Rust-level implementation.

## See Also

- [generic-native-functions.md](generic-native-functions.md) — how `#[sa_fn]` encodes SA type signatures on native functions
- [generics-implementation.md](generics-implementation.md) — the type checker unification this depends on for generic functions
- [`src/structured-agent/src/compiler/mod.rs`](../src/structured-agent/src/compiler/mod.rs) — `Compiler` struct
- [`src/structured-agent/src/compiler/sigs.rs`](../src/structured-agent/src/compiler/sigs.rs) — `collect_sigs`
- [`src/structured-agent/src/runtime/engine.rs`](../src/structured-agent/src/runtime/engine.rs) — `RuntimeBuilder`
- [`src/structured-agent-stdlib/src/unstable/`](../src/structured-agent-stdlib/src/unstable/) — the unstable module