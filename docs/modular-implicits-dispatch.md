# Modular Implicits Dispatch

Constrained generics in SA — `fn sum<T: Add>(a: T, b: T): T` — require a mechanism to route a trait method call inside the generic body to the correct implementation at runtime. The chosen strategy is modular implicits, in which trait bounds compile into implicit module arguments resolved by the compiler at each call site. The theoretical basis and the comparison with alternative strategies (monomorphisation, dictionary passing, witness tables, and others) are in [constrained-generics.md](constrained-generics.md), Appendix B. This document records the concrete design decisions that turn that strategy into working bytecode.

The work divides into two phases. The first addresses foundational architectural issues in the current codebase that would otherwise make the dispatch mechanism fragile or inconsistent. The second builds the dispatch mechanism itself on top of that foundation. Neither phase has been implemented yet.

## The Naming Problem

The most significant architectural issue is that function names are currently bare strings with no consistent structure. The entry module registers functions under unqualified names (`"add"`, `"main"`). Library modules register functions under `"module::fn"`. There is no single formula for deriving a function's registry key from its origin.

This breaks immediately for impl functions, which require a four-part identity: the module that defines the impl, the type being implemented for, the trait being satisfied, and the function name. `"Vec2::Add::add"` is correct for an impl in the entry module; `"mylib::Vec2::Add::add"` is correct for the same impl in a library module. A string-concatenation convention cannot be made consistent without first making all function naming consistent.

The decision is to represent function identity as a structured type rather than a string:

```structured-agent/src/structured-agent/src/names.rs#L1-12
pub struct FunctionName {
    pub module: String,
    pub kind: FunctionNameKind,
}

pub enum FunctionNameKind {
    Function { name: String },
    Impl { type_name: String, trait_name: String, name: String },
}
```

`FunctionName` derives `Hash`, `Eq`, and `PartialEq` so it can serve as a `HashMap` key directly. Its `Display` implementation produces `"module::name"` for plain functions and `"module::TypeName::TraitName::name"` for impl functions. The enum enforces that `type_name` and `trait_name` are always both present or both absent — a constraint that a string convention cannot express.

`FunctionName` lives in a new `src/names.rs` module with no dependencies on other SA modules, making it available throughout the codebase without introducing import cycles.

## Entry Module Naming

A complementary decision is that all functions, including those in the entry module, carry a module prefix. The entry module is always named `"main"`. A function `add` in the entry module is registered under `FunctionName { module: "main", kind: Function { name: "add" } }`.

Previously, entry module functions were registered without a prefix. The asymmetry meant that `emit_module` applied `"module::"` only for non-entry modules:

```structured-agent/src/structured-agent/src/compiler/mod.rs#L307-308
let prefix = (!parsed.is_entry).then_some(parsed.name.as_str());
```

This single line is the source of the inconsistency. Removing the conditional and always supplying `"main"` for the entry module makes every function name follow the same formula.

Existing call sites that resolve function names as short strings continue to work through the alias layer described below.

## The Alias Layer

The current alias mechanism in `apply_pending_aliases` inserts a full clone of each `CompiledFunction` under the alias key. There is no indirection: aliases and canonical entries are indistinguishable in the `functions` map, and the VM performs a single exact-match lookup with no resolution step.

The replacement is a separate `aliases: HashMap<FunctionName, FunctionName>` field on `CompiledProgram`. The `functions` map holds exactly one copy of each function under its canonical `FunctionName`. `CompiledProgram` exposes a single resolution point:

```structured-agent/src/structured-agent/src/compiler/mod.rs#L1-5
pub fn resolve(&self, name: &FunctionName) -> Option<&CompiledFunction> {
    self.functions.get(name)
        .or_else(|| self.aliases.get(name)
            .and_then(|canonical| self.functions.get(canonical)))
}
```

The VM calls `resolve` in place of the bare registry lookup. For entry module functions, thin aliases from the bare name to the `"main::"` prefixed canonical name are registered automatically, so intra-module call sites continue to work without change.

## Emitting Impl Functions

`TraitImpl` definitions are currently dropped in `emit_module` and produce no bytecode. This is the most immediate consequence of the naming problem: without a consistent naming scheme there was nowhere reliable to register impl functions. With `FunctionName::Impl` established, `emit_module` can emit each function from a `TraitImpl` block under its canonical name.

The `Self` type used in trait declarations (`fn add(self: Self, other: Self): Self`) is substituted for the concrete implementing type when registering impl function signatures in the type checker. The impl body source may use `Self`, and the type checker substitutes `Self → TypeName` during signature registration and body checking.

## Vtable Rewriting and FunctionKind

`lower_typed_module`, which rewrites `resolved` in typed AST call nodes via the module vtable, currently visits only `Function` definitions. It also fails to update the `kind` field on `Call` nodes when `resolved` changes, meaning that a vtable substitution can silently emit `CallBytecode` for a function that is actually external, or vice versa.

Two corrections are needed. First, `lower_typed_module` must also traverse `TraitImpl` function bodies, since impl functions can themselves call other functions that require vtable substitution. Second, the function kinds map returned by `type_check_module` (currently discarded as `_` at the call site in `compile`) must be passed into `lower_typed_module` so that `kind` is updated alongside `resolved` when a rewrite occurs.

## The Dispatch Mechanism

With the naming foundation in place, the dispatch mechanism itself has a clear structure. A bounded type parameter `T: Add` on a function compiles into an implicit module parameter named `T_Add`. The naming convention is `TypeVar_TraitName`; for multiple bounds (`T: Add + Sub`) there are two parameters, `T_Add` and `T_Sub`. For multiple type parameters sharing a bound (`T: Add, U: Add`) the names `T_Add` and `U_Add` disambiguate.

At a call site `sum(v1, v2)` where the type checker has resolved `T = Vec2` and verified `Vec2 ∈ impls[Add]`, the call node in the typed AST carries the resolved implicit module alongside the ordinary arguments. The bytecode emitter loads the module identity as a module value — a first-class `ExpressionValue::Module(FunctionName)` — into a variable and passes it as a leading parameter to the callee.

A module value carries a `FunctionName` directly. It is not a string that must be parsed or concatenated at runtime; the fully structured identity is baked in at compile time. The new `LoadModule` instruction creates one:

```structured-agent/src/structured-agent/src/bytecode/instruction.rs#L1-4
LoadModule {
    name: FunctionName,
    dest: String,
}
```

Inside the body of `sum`, a call `add(a, b)` where `a: T` is detected by the type checker as a trait method call dispatched through the implicit `Add` module. It is emitted as a `CallIndirect` instruction:

```structured-agent/src/structured-agent/src/bytecode/instruction.rs#L1-6
CallIndirect {
    module_param: String,
    fn_name:      String,
    params:       Vec<String>,
    dest:         String,
}
```

`module_param` is the name of a variable holding a `Module` value. The VM reads that value, extracts the `FunctionName`, appends the bare function name as the `name` field, and resolves the result through the registry. No string construction occurs at runtime; the `FunctionName` was fully determined at compile time.

The following diagram shows the full path from source call to execution:

```
sum(v1, v2)                          [SA source, T inferred as Vec2]
    │
    ▼ type checker
Call { resolved: main::sum,
       impl_modules: [T_Add → FunctionName::Impl {
           module: "main", type_name: "Vec2", trait_name: "Add" }],
       arguments: [v1, v2] }
    │
    ▼ bytecode compiler
LoadModule  T_Add, main::Vec2::Add
CallBytecode  main::sum, [T_Add, v1, v2], dest
    │
    ▼ VM — enters main::sum, declares T_Add as Module value in child context
    │
    ▼ inside main::sum body
CallIndirect  T_Add, "add", [a, b], result
    │
    ▼ VM — reads T_Add → Module(FunctionName::Impl {
    │           module: "main", type_name: "Vec2",
    │           trait_name: "Add", name: "add" })
    ▼
main::Vec2::Add::add(a, b)
```

## Syntax

In the short term, trait method calls inside generic bodies use the same function-call syntax as ordinary calls: `add(a, b)`. The type checker detects that `add` is declared in the `Add` trait and that a bound `T: Add` is in scope, and routes the call through the implicit module parameter.

The longer-term form is `a.add(b)`, treating the first argument as the receiver. When method call syntax is added to the parser it will desugar to the same underlying call, and `a + b` will in turn desugar to `a.add(b)`. The dispatch mechanism is identical in all three forms.

## The Type Checker and Module Identity

The type checker currently has no knowledge of which module it is checking. To produce fully qualified `FunctionName` values in the typed AST, the module name must be passed into `check_module` as a parameter. This is a small, targeted addition. The broader shift towards a query-based type checker that sees the whole program simultaneously — at which point module identity will be part of the query context — makes this a transitional measure rather than a permanent design. The query-based architecture is described in [type-system-abstractions.md](type-system-abstractions.md).

`MetaFunction { function_name: String }` in the instruction set is a display name for LLM context generation, not a registry identity. It is not changed to `FunctionName`.

## Coherence

The same-module restriction — `impl Vec2: Add` must live in the module that defines `struct Vec2` — means that for any concrete type and trait, at most one `FunctionName::Impl` key exists in the registry. The `FunctionName` struct makes this structural: the registry key encodes (module, type, trait, fn) as a single typed value, and duplicate registrations are a compile-time error at the definition site. The coherence implications at scale, and the deferred cases (blanket impls, third-party impls, overlapping instances), are discussed in [constrained-generics.md](constrained-generics.md), Appendix A.

## Implementation Phases

The work is split into a foundational phase that corrects existing architectural debt and four subsequent phases that build the dispatch mechanism on top of it.

### Phase 0 — Foundation - COMPLETE

Phase 0 has no user-visible behaviour change. Its purpose is to make the subsequent phases straightforward rather than brittle.

The first step introduces the `FunctionName` struct in a new `src/names.rs` module. It has no dependencies on other SA modules. All other steps in this phase depend on it.

The second step threads `FunctionName` through the codebase as a mechanical substitution: `CompiledFunction::name`, `Instruction::CallBytecode { function_name }`, `Instruction::CallExternal { function_name }`, and `typed_ast::Expression::Call::resolved` all change from `String` to `FunctionName`. No behaviour changes; the `Display` implementation produces the same strings the code used before.

The third step replaces `apply_pending_aliases` with a thin alias map. `CompiledProgram` gains `aliases: HashMap<FunctionName, FunctionName>` and loses the function-cloning logic. The VM resolves names through `CompiledProgram::resolve` rather than a bare registry lookup.

The fourth step makes all function names fully qualified. The entry module prefix changes from absent to `"main"`. Thin aliases from the bare function name to the `"main::"` prefixed canonical name are registered automatically for every entry module function. The `main_function` field tracks `"main::main"`. The module name is passed into `check_module` as a new parameter so the type checker can produce fully qualified `FunctionName` values.

The fifth step adds the module value type. `ExpressionValue` gains a `Module(FunctionName)` variant in the runtime. A new `LoadModule { name: FunctionName, dest: String }` instruction loads a module reference into a variable. This is the correct representation for implicit module parameters: the `FunctionName` is fully determined at compile time and carried as a typed value, not encoded as a string to be parsed or concatenated at runtime. This also gives the module hot-swap mechanism in durable execution a proper handle — replacing a module means replacing the value, not hunting string keys.

The sixth step stops discarding `TraitImpl` definitions in `emit_module`. Each function in an impl block is emitted under a `FunctionName::Impl` key. The type checker substitutes `Self` for the concrete type name when registering impl function signatures.

The seventh step fixes `lower_typed_module`. It is extended to traverse `TraitImpl` function bodies in addition to `Function` bodies. The function kinds map returned by `type_check_module` — currently discarded as `_` at the call site — is passed into `lower_typed_module` so that the `kind` field on each `Call` node is updated when `resolved` is rewritten by vtable substitution.

### Phase 1 — Impl Functions Callable - NOT STARTED

With Phase 0 in place, impl functions are emitted but the type checker does not yet resolve unqualified trait method calls to their qualified names. Phase 1 completes the type checker side: `collect_function_signatures` registers impl functions under `FunctionName::Impl` qualified names with `Self` substituted, and `check_call` is updated to recognise unqualified trait method names and resolve them to the correct qualified function when the argument types are concretely known.

At the end of this phase, `add(v1, v2)` where `v1` and `v2` have a known concrete type that implements `Add` resolves to the correct impl function and executes. No new bytecode instructions are required.

### Phase 2 — Generic Dispatch - NOT STARTED

Phase 2 implements the core of modular implicits: dispatch through implicit module parameters for generic functions.

In the type checker, calls to functions with bounded type parameters gain an `impl_modules` field on the typed AST `Call` node, recording the resolved module identity for each bound — for example `[("T_Add", FunctionName::Impl { module: "main", type_name: "Vec2", trait_name: "Add" })]` when `T` is resolved to `Vec2` and `T: Add`. Inside generic function bodies, calls to trait methods where the receiver type is a type variable are emitted as a new `TraitMethodCall` typed AST variant rather than a plain `Call`.

The bytecode compiler emits a `LoadModule` instruction for each implicit module parameter before the ordinary arguments, and passes them as leading parameters in `CallBytecode`. `TraitMethodCall` nodes compile to the new `CallIndirect` instruction, which names the implicit module parameter variable and the bare function name.

The VM, on entering a function call with implicit module parameters, declares the corresponding `Module` value variables in the child context before execution begins. `CallIndirect` is handled by reading the module parameter variable, extracting its `FunctionName`, appending the bare function name, and resolving the result through the registry.

At the end of this phase, `fn combine<T: Add>(a: T, b: T): T { return add(a, b) }` called with a concrete type executes the correct impl function.

### Phase 3 — Method Call Syntax - NOT STARTED

The parser desugars `a.add(b)` to `add(a, b)`. Phase 2 handles the dispatch. No changes to the type checker, bytecode, or VM are required.

Operator overloading — `a + b` desugaring to `a.add(b)` — is a further parser change that follows the same pattern and is kept as a separate subsequent step.

## Deferred

Generic structs (`struct Pair<A, B>`) require orthogonal changes to struct definition, instantiation, and field access and remain deferred.

## See Also

- [constrained-generics.md](constrained-generics.md) — trait declarations, bounds, coherence, and dispatch strategy comparison
- [generics-implementation.md](generics-implementation.md) — parametric polymorphism without bounds, the unification and substitution foundation this work extends
- [type-system-abstractions.md](type-system-abstractions.md) — the query-based type checker direction
- [module-system.md](module-system.md) — sigs, module parameters, and wiring
- [bytecode-architecture.md](bytecode-architecture.md) — the instruction set and VM execution model
- White, L., Bour, F., Yallop, J. (2015). "Modular Implicits." ML Workshop 2014. https://arxiv.org/abs/1512.01438
- Wadler, P. and Blott, S. (1989). "How to Make Ad-Hoc Polymorphism Less Ad Hoc." POPL 1989. https://dl.acm.org/doi/10.1145/75277.75283
