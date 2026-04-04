# Generic Native Functions

The SA macro system — `#[sa_fn]` and `#[sa_module]` — generates `NativeFunction` implementations from plain Rust functions. Until now it has supported only concrete types: `String`, `bool`, `i64`, `Vec<T>`, and `Option<T>` where the inner type is itself concrete. This document describes the extension of the macro to support generic type parameters, enabling stdlib functions such as `head`, `tail`, `is_some`, and `some_value` to be declared once rather than once per element type.

The change is motivated directly by the generics implementation described in [generics-implementation.md](generics-implementation.md). The SA type checker can now unify type variables at call sites; native functions declared with `type_params` in source-level `extern fn` declarations benefit from this. The macro extension makes the native function's own SA-level type signature consistent with that declaration.

## The Problem

`head` and `tail` are currently hardcoded to `List<String>` in `structured-agent-stdlib/src/unstable/`. The type checker accepts calls to `head` with a `List<String>` argument but rejects `List<Int>`. The `is_some` and `some_value` functions exist as four separate concrete variants: `is_some_string`, `is_some_list`, `some_value_string`, `some_value_list`. Adding a new element type requires new variants throughout.

The root cause is that `#[sa_fn]` has no way to declare a type variable. Its type mapper rejects any identifier it does not recognise as a concrete Rust type.

## Design

### `type_params` attribute argument

`#[sa_fn]` is extended to accept a `type_params` argument listing the SA-level type variable names:

```/dev/null/example.rs#L1-3
#[sa_fn(type_params = "T")]
fn head<T: Clone>(list: Vec<T>) -> Option<T> {
```

Multiple type parameters are comma-separated: `type_params = "T, U"`.

### Type mapping

The type mapper in `types.rs` receives the set of declared type params alongside the Rust `syn::Type`. Before the concrete-type matches, any bare identifier that appears in the type params set is mapped to `Type::generic(name)` rather than producing a compile error. `Vec<T>` where `T` is a type param maps to `Type::list(Type::generic("T"))`. `Option<T>` maps to `Type::option(Type::generic("T"))`.

### Argument extraction

The generated `execute` body instantiates each type param as `ExpressionValue`. Extraction rules for generic types:

| Rust parameter type | Generated extraction |
|---|---|
| `Vec<T>` (T is type param) | `args[i].as_list_elements()?` → `Vec<ExpressionValue>` |
| `Option<T>` (T is type param) | `args[i].as_option()?` → `Option<ExpressionValue>` |
| bare `T` | `args[i].clone()` → `ExpressionValue` |

`as_list_elements` is a new method on `ExpressionValue` that iterates the underlying `ListArray` and collects its elements into a `Vec<ExpressionValue>`.

### Return value conversion

| Rust return type | Generated conversion |
|---|---|
| `Option<T>` (T is type param) | `None` → `option_none()`, `Some(v)` → `option_some(v)` |
| `Vec<T>` (T is type param) | `ExpressionValue::from_elements(__sa_result)?` |
| bare `T` | `__sa_result` returned directly |

### Generated struct

The generated `{PascalCase}Function` struct gains a `type_params: Vec<String>` field populated at construction time. The `NativeFunction` trait gains a `type_params` method with a default implementation returning `&[]`; the macro emits a concrete implementation for functions that declare type params.

## Files Changed

| File | Change |
|---|---|
| `structured-agent-runtime/src/types.rs` | Add `Generic(String)` to `Type` enum; add `Type::generic(name)` constructor; add `type_params` method to `NativeFunction` trait with default `&[]` |
| `structured-agent-runtime/src/expression.rs` | Add `as_list_elements() -> Result<Vec<ExpressionValue>, String>` |
| `structured-agent-macros/src/types.rs` | Accept `type_params: &[String]` context; map type param idents to `Type::generic(name)` |
| `structured-agent-macros/src/extract.rs` | Handle `Vec<T>`, `Option<T>`, and bare `T` when T is a type param |
| `structured-agent-macros/src/convert.rs` | Handle `Vec<T>` and bare `T` return types |
| `structured-agent-macros/src/fn_gen.rs` | Parse `type_params` from attribute; add field and `NativeFunction::type_params` impl to generated struct |
| `structured-agent-stdlib/src/unstable/` | Rewrite `head`, `tail`, `is_some`, `some_value` using `#[sa_fn(type_params = "T")]`; remove the four monomorphic variants |

## Stdlib Functions After the Change

```/dev/null/stdlib.rs#L1-16
#[sa_fn(type_params = "T")]
fn head<T: Clone>(list: Vec<T>) -> Option<T> {
    list.first().cloned()
}

#[sa_fn(type_params = "T")]
fn tail<T: Clone>(list: Vec<T>) -> Option<Vec<T>> {
    if list.is_empty() { None } else { Some(list[1..].to_vec()) }
}

#[sa_fn(type_params = "T")]
fn is_some<T>(value: Option<T>) -> bool { value.is_some() }

#[sa_fn(type_params = "T")]
fn some_value<T>(value: Option<T>) -> T { value.unwrap() }
```

At code-generation time, `T` is instantiated as `ExpressionValue`. The SA type checker — which does not use `NativeFunction` at all, working instead from source-level `extern fn` declarations — resolves `T` to the concrete element type at each call site via the unification mechanism described in [generics-implementation.md](generics-implementation.md).

## Relationship to the Type Checker

The type checker learns native function signatures exclusively from `extern fn` declarations in SA source files, not from `NativeFunction` at runtime. The macro change has no direct effect on the type checker. Its value is threefold: it removes the monomorphic variants from the stdlib, ensures the native function's own reported SA signature (`NativeFunction::type_params`, `NativeFunction::return_type`) is consistent with the source-level declaration, and makes adding further generic stdlib functions straightforward.

## Future Work

Once generic structs and sum types are in place, `Option<T>` and `List<T>` can move from compiler built-ins to the standard library. At that point the `is_some`, `some_value`, `head`, and `tail` functions defined here become ordinary SA functions rather than native ones, and this machinery is no longer needed for them. The macro extension remains useful for any native function that genuinely requires Rust-level implementation over generic types.

## See Also

- [generics-implementation.md](generics-implementation.md) — function-level generics in the SA type checker
- [`src/structured-agent-macros/src/`](../src/structured-agent-macros/src/) — macro source
- [`src/structured-agent-stdlib/src/unstable/`](../src/structured-agent-stdlib/src/unstable/) — stdlib unstable module
- [`src/structured-agent-runtime/src/types.rs`](../src/structured-agent-runtime/src/types.rs) — `NativeFunction` trait and `Type` enum
- [`src/structured-agent-runtime/src/expression.rs`](../src/structured-agent-runtime/src/expression.rs) — `ExpressionValue`
