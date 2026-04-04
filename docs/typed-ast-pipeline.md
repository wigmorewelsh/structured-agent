# Typed AST Pipeline

The SA compiler currently threads three separate data structures into its bytecode emission stage: the original `ast::Module`, a `HashMap<String, FunctionKind>` produced by the type checker as a side-channel, and a `Vtables` map produced by a separate wiring pass. None of the type information computed during checking — the resolved type of every expression, the resolved target of every call, the concrete module behind every parameter — survives into the emitter. The emitter re-derives what it can from the undecorated AST and receives the rest through the side-channels.

Moving to a typed AST resolves this by making the type checker's output a first-class artefact: a new `typed_ast::Module` in which every expression carries its resolved type and every call carries its resolved target. The bytecode emitter reads from this single structure and requires no side-channels. The pipeline becomes a strict sequence of transformations, each consuming the output of the one before.

This change is the prerequisite for generics, for proper module contract checking, and for the `in M` context restriction described in [module-system.md](module-system.md).

## The Current Pipeline

```
Source
  → parse           → ast::Module
  → collect_sigs    → SigTable
  → type_check      → HashMap<String, FunctionKind>   (side-channel)
  → resolve_vtables → Vtables                          (side-channel)
  → emit_module(ast::Module, kinds, vtables)
  → CompiledProgram
```

The type checker in [`typecheck/checker.rs`](../src/structured-agent/src/typecheck/checker.rs) traverses every expression, computes its type, checks it, and discards the result. The bytecode compiler in [`bytecode/compiler.rs`](../src/structured-agent/src/bytecode/compiler.rs) reads the original `ast::Function` and uses the `kinds` map to determine how to emit each call. The vtable map, built in [`compiler/wiring.rs`](../src/structured-agent/src/compiler/wiring.rs) from a separate traversal of the raw AST, is stored on `CompiledProgram` and resolved by the VM at runtime.

The consequence is that information the type checker has already computed must be recomputed, looked up in a side-channel, or deferred to runtime. Adding generics makes this untenable: a type variable substitution resolved at the call site has nowhere to live. The same is true for `in M` context resolution and for the sig conformance result in Phase 6 of the module system.

## The Typed AST

A typed AST is a tree whose expression nodes carry type annotations. The canonical form, described in Pierce's *Types and Programming Languages* (ch. 9, MIT Press 2002), has each expression node decorated with the type the type checker assigned to it. Harper's *Practical Foundations of Programming Languages* (ch. 47, Cambridge 2016) describes type-directed compilation as the practice of driving code generation from these annotations rather than re-inferring types during emission.

For SA, the typed AST mirrors [`ast/mod.rs`](../src/structured-agent/src/ast/mod.rs) with two additions. Every `Expression` carries a `ty: ast::Type` field holding its resolved type. Every `Call` expression carries a `resolved: String` field holding the fully-qualified function name after alias resolution, alongside the `FunctionKind` indicating whether the target is bytecode or a native function. The rest of the structure — statements, function bodies, definitions — is identical to the untyped AST.

A short example illustrates the difference. In the untyped AST, a call expression is:

```/dev/null/untyped.rs#L1-5
Expression::Call {
    function: "read",
    arguments: vec![...],
    span,
}
```

In the typed AST, the same node becomes:

```/dev/null/typed.rs#L1-8
TypedExpression::Call {
    function: "read",
    resolved: "storage::read",
    kind: FunctionKind::External,
    arguments: vec![...],
    ty: Type::Option(Box::new(Type::String)),
    span,
}
```

The emitter reads `resolved`, `kind`, and `ty` directly. Nothing is looked up at emit time.

## Migration Phases

The migration is split into four phases, each a self-contained change that leaves all existing tests passing. The phases are ordered so that the later, more disruptive changes to the module system are deferred until the core pipeline has been proven.

### Phase 1: Add `typed_ast` — DONE

A new module, `typed_ast/mod.rs`, mirrors the structure of `ast/mod.rs` with the additions described above. The type checker is extended to build a `TypedModule` as a by-product of its existing traversal: since `check_expression` already computes and returns a type for each node, it can construct the corresponding `TypedExpression` at the same time.

The return type of `TypeChecker::check_module_with_external_sigs` changes from `Result<HashMap<String, FunctionKind>, TypeError>` to `Result<(TypedModule, HashMap<String, FunctionKind>), TypeError>`. The `BytecodeCompiler` still reads `ast::Function` at this stage; the `TypedModule` is threaded through the orchestrator in [`compiler/mod.rs`](../src/structured-agent/src/compiler/mod.rs) but not yet consumed. This phase is purely additive.

### Phase 2: Switch Bytecode Emission to `TypedFunction` — DONE

`BytecodeCompiler::compile_to_bytecode` is changed to accept `&typed_ast::Function` instead of `&ast::Function`. The `emit_module` function's signature changes accordingly. All methods on `BytecodeCompiler` that previously read expression types from the `kinds` map now read them from `TypedExpression::ty`. The `ast::Module` argument is removed from `emit_module`; it takes a `&typed_ast::Module` instead.

### Phase 3: Remove the `kinds` Side-Channel — DONE (subsumed by Phase 2)

With the resolved function name and `FunctionKind` embedded on `TypedExpression::Call`, the `kinds: HashMap<String, FunctionKind>` field on `BytecodeCompiler` is no longer needed. It is removed. The `BytecodeCompiler::new` constructor takes no arguments. The orchestrator no longer threads `module_kinds` into `emit_module`.

All three changes were necessary in Phase 2 to avoid dead-code warnings: once `compile_call_expression` read `kind` from the typed expression, `self.kinds` became unused, which made `new(kinds)` pointless, which made the `module_kinds` collection in `compile()` pointless. The chain collapsed together.

### Phase 4: Unify Vtable Resolution with the Typed AST — DONE

The vtable map is currently built by `resolve_vtables` in [`compiler/wiring.rs`](../src/structured-agent/src/compiler/wiring.rs) from a traversal of the raw AST, before type checking runs. The result is stored on `CompiledProgram` and resolved by the VM at runtime. This is the last remaining side-channel.

In this phase, vtable resolution becomes a lowering pass that consumes a `TypedModule` and produces a `ResolvedModule`, replacing each `Call` to a module parameter (e.g. `io::read`) with its concrete target (e.g. `storage::read`). The `Vtables` map is removed from `CompiledProgram`. The VM performs no vtable lookup.

This phase is also where Phase 6 of the module system — contract matching — is implemented. As the resolver substitutes each module parameter, it verifies the concrete module satisfies the required sig, reporting `TypeError` through the same path as the type checker. The two concerns are unified because they share the same traversal: for each wiring site, resolve the concrete module and check it satisfies the contract.

## Relationship to Generics

Parametric polymorphism requires that type variable substitutions resolved at a call site are recorded somewhere and carried through to code generation. In a compiler without a typed AST, there is nowhere to put them. The type checker resolves `T = String` at the call site of `head(["a", "b"])` and then discards the result; the bytecode emitter later encounters `Type::Generic("T")` in the original AST and cannot proceed.

With the typed AST, the substitution is embedded in the `TypedExpression` for the call. The emitter sees `Type::List(String)` as the argument type and `Type::Option(String)` as the return type; the type variable is gone. The runtime, which already uses `ExpressionValue` as a uniform representation for all values, requires no changes. Generics are, from the emitter's perspective, a non-event.

The type checker gains a unification step — when it encounters a call to `head`, it unifies `T` with the element type of the argument list, substitutes throughout the return type, and writes the substituted types into the `TypedExpression` nodes it constructs. The formal basis for this is the Hindley-Milner algorithm (Damas and Milner, "Principal type-schemes for functional programs", *POPL* 1982), though SA does not require full inference: type variables appear only in explicitly generic function signatures and are resolved at call sites.

## Relationship to the Module System and `in M`

Module parameters are structurally the same as generic type variables, at module granularity rather than function granularity. A module declared `mod db(io: storage.Storage)` is parameterised over its `io` dependency in exactly the sense that `head<T>` is parameterised over its element type. Phase 4 of the typed AST migration is therefore the module-system analogue of generic type variable substitution: the resolver substitutes the concrete module for the parameter and records the result in the typed AST.

The `in M` constraint — `fn f(): T in M` — declares that the generated function's free variables are drawn from the exports of `M`. The type checker resolves `M` to its concrete export set and records it on the typed function node. The bytecode emitter reads this set and passes it to the LLM as the generation context. Without a typed AST, this resolution has nowhere to live between the type checker and the emitter.

`deferred T`, the modal type for LLM-generated values, follows the same pattern. The type checker annotates which expressions cross the generation boundary; the typed AST carries those annotations to the emitter, which generates the appropriate instructions. The formal grounding is Contextual Modal Type Theory (Nanevski, Pfenning, Pientka, "Contextual Modal Type Theory", *ACM Transactions on Computational Logic*, 2008; free PDF at https://www.cs.cmu.edu/~fp/papers/tocl07.pdf), whose substitution rule — instantiating a context `M` to produce a value of type `A` — is what the typed AST makes concrete.

## Summary of Pipeline Changes by File

| File | Phase |
|---|---|
| `typed_ast/mod.rs` | 1: new module mirroring `ast/mod.rs` with `ty` on expressions and `resolved`/`kind` on calls |
| `typecheck/checker.rs` | 1: build `TypedModule` alongside type checking; 4: resolve module parameter call targets |
| `compiler/mod.rs` | 1: thread `TypedModule` through orchestrator; 2: pass to `emit_module`; 3: remove `module_kinds`; 4: run vtable resolution as a lowering pass |
| `bytecode/compiler.rs` | 2: accept `TypedFunction` instead of `ast::Function`; 3: remove `kinds` field |
| `compiler/wiring.rs` | 4: replaced by a typed AST lowering pass |
| `bytecode/vm.rs` | 4: vtable resolution removed |
| `runtime/engine.rs` | 4: `vtables` removed from `CompiledProgram` |

## See Also

- [module-system.md](module-system.md) — the full module system design, including `in M` and `deferred`
- [module-system-implementation.md](module-system-implementation.md) — phase-by-phase implementation record, including Phase 6 (contract matching)
- [type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — CMTT, taint tracking, and the `in M` formal model
- Pierce, B. C. (2002). *Types and Programming Languages*. MIT Press. Chapters 9 (simply typed lambda calculus) and 22 (type reconstruction).
- Harper, R. (2016). *Practical Foundations of Programming Languages* (2nd ed.). Cambridge University Press. Chapter 47 (type-directed compilation). Free PDF at https://www.cs.cmu.edu/~rwh/pfpl/
- Damas, L. and Milner, R. (1982). "Principal type-schemes for functional programs." *POPL 1982*. https://dl.acm.org/doi/10.1145/582153.582176
- [`src/structured-agent/src/ast/mod.rs`](../src/structured-agent/src/ast/mod.rs) — the untyped AST
- [`src/structured-agent/src/typecheck/checker.rs`](../src/structured-agent/src/typecheck/checker.rs) — the type checker
- [`src/structured-agent/src/bytecode/compiler.rs`](../src/structured-agent/src/bytecode/compiler.rs) — the bytecode emitter
- [`src/structured-agent/src/compiler/wiring.rs`](../src/structured-agent/src/compiler/wiring.rs) — vtable resolution