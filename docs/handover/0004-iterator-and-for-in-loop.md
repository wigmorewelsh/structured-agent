# Iterator and For-In Loop

## Goal

Add an iterator protocol and `for x in expr { }` syntax to the SA language. The design follows .NET's `IEnumerator<T>` pattern: an `Iterator<T>` trait with `move_next(): Boolean` and `current(): T`, decoupled from `List`. A separate `Iterable<T>` trait (Chunk 4, not started) would allow implicit `.iter()` calls in `for` loops.

## What Has Been Done

### Chunk 1 — Iterator runtime (committed)

`ListIteratorValue` was added to `structured-agent-runtime` as a new runtime value type at [src/structured-agent-runtime/src/runtime_value/list_iterator.rs](../../src/structured-agent-runtime/src/runtime_value/list_iterator.rs). It wraps `Arc<Mutex<ListIteratorState>>` where state holds an `Arc<ListArray>` and an `Option<usize>` index. Clone shares the Arc, so cloned values share mutable iteration state. Equality is identity via `Arc::ptr_eq`.

`ListValue` gained a `list_arc()` accessor. `ExpressionValue` gained `list_iterator()` and `as_list_iterator()` constructors.

`IteratorModule` was added at [src/structured-agent-stdlib/src/iterator.rs](../../src/structured-agent-stdlib/src/iterator.rs). It is hand-written (not using the `#[sa_module]` macro) because the macro's `map_type_to_runtime` only treats `Self` as a generic type parameter and cannot express `T` as a trait-level generic. The module declares the `Iterator` trait with `move_next(self: Self): Boolean` and `current(self: Self): T` (where `T` is `Type::generic("T")`), and registers a `ListIterator implements Iterator` native impl.

### Chunk 2 — Parser and AST (committed)

`Statement::ForIn { variable, iterable, body, span }` was added to the AST at [src/structured-agent-ast/src/ast/mod.rs](../../src/structured-agent-ast/src/ast/mod.rs) and the matching typed variant at [src/structured-agent-typed-ast/src/lib.rs](../../src/structured-agent-typed-ast/src/lib.rs). The parser at [src/structured-agent-parser/src/lib.rs](../../src/structured-agent-parser/src/lib.rs) parses `for <ident> in <expr> { <stmts> }` using `attempt((lex_string("for"), identifier(), lex_string("in")))` for correct backtracking. Stub arms were added to every exhaustive match across the analysis, typecheck, and compiler crates.

### Chunk 3 — Type checker, bytecode compiler, and calling convention fix (committed)

The following files were modified to complete chunk 3:

- `src/structured-agent-typecheck/src/db.rs` — `find_impl_fn` had its `i.module == *current_module` filter removed so that stdlib trait method calls are visible from user code.
- `src/structured-agent-typecheck/src/synthesize.rs` — `check_statement` for `ForIn`: synthesises the iterable expression type, checks for a matching `Iterator` impl, and type-checks the body with the loop variable bound to the element type. The element type is extracted from `RT::Parameterized(_, args)` when available, falling back to `RT::Generic("T")`.
- `src/structured-agent-typecheck/src/elaboration.rs` — `elaborate_statement` for `ForIn`: elaborates the iterable, finds the impl key by scanning the impl table directly, derives `move_next_fn` and `current_fn` paths via `DefinitionPath::for_impl_fn`, and elaborates the body with the loop variable in a child scope.
- `src/structured-agent-typed-ast/src/lib.rs` — `Statement::ForIn` extended with `move_next_fn: DefinitionPath` and `current_fn: DefinitionPath` fields.
- `src/structured-agent-bytecode-compiler/src/compiler.rs` — `compile_for_in_statement` desugars to an explicit loop: `CallBytecode(move_next)` / `BrFalse` / `CallBytecode(current)` / body / `Br`, wrapped in `CtxChild` / `CtxRestore`.
- `src/structured-agent-stdlib/src/iterator.rs` — `iter` is an inherent impl on `List` in `native_impls()`. All three function bodies (`iter`, `move_next`, `current`) follow the calling convention described below and each ends with `Ret { var: Slot(0) }`.
- `src/structured-agent-il/src/module_trait.rs` — `NativeImplDecl` gained a `type_params: Vec<String>` field.
- `src/structured-agent-macros/src/module_gen.rs` — updated to emit `type_params: vec![]` for generated `NativeImplDecl` constructions.
- `src/structured-agent/src/compiler/mod.rs` — the slot table builder for native impl `BytecodeRef` was updated to include type params before parameters, matching the behaviour for native functions.
- Three end-to-end integration tests at [src/structured-agent/tests/integration/for_in_test.rs](../../src/structured-agent/tests/integration/for_in_test.rs) cover single element, multiple elements, and two-element lists.

## Calling Convention for Native Impl Functions

The slot layout follows the documented convention in [docs/function-signatures.md](../function-signatures.md):

```
[Slot(0)=__ret, TypeParams..., ModuleParams..., Implicits..., self, UserArgs...]
```

For `iter(self: List<T>): ListIterator<T>` with one type param `T`:

```
[Slot(0)=__ret, Slot(1)=T, Slot(2)=self]
CallNative { params: [Slot(2)], dest: Slot(0) }
Ret { var: Slot(0) }
```

For `move_next(self: Self): Boolean` and `current(self: Self): T` with no type params:

```
[Slot(0)=__ret, Slot(1)=self]
CallNative { params: [Slot(1)], dest: Slot(0) }
Ret { var: Slot(0) }
```

The elaboration inserts a `TypeLiteral` argument for each declared type param in any `Bytecode` function. The compiler's slot table builder for native impls must therefore add type param slots before parameter slots, or the argument count will not match the frame size.

## Deferred Work

Chunk 4 (the `Iterable<T>` trait and implicit `.iter()` insertion in the type checker) has not been started. Until it is, SA code must write `for x in items.iter()` rather than `for x in items`.

The `TypeDefinitionKind::Trait` variant in [src/structured-agent-runtime/src/symbols.rs](../../src/structured-agent-runtime/src/symbols.rs) has no `generic_parameters` field. The `Iterator<T>` trait is therefore not truly generic in the type system — `T` in `current(): T` is a name convention rather than a declared parameter. In practice the element type does propagate correctly because `iter()` returns `ListIterator<T>` parameterised on the list element type, and the type checker extracts that argument. The limitation only bites if `Iterator` is used on a type whose element type cannot be inferred from the iterable expression alone.

There is no integration test for an empty list (loop body never entered). That path is exercised by the unit tests in `iterator.rs` but not at the SA language level.
