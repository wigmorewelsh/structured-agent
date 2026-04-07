# Symbol Table Roadmap

## Direction

The `MetaData<R>` type is generic over a `References` implementation by design. The intent is that
the same table structure carries the program through every phase:

```
AST → TypeChecker → MetaData<TypedRefs>     (ast_ref = Arc<typed_ast::Function>)
                         │
                         ▼
              BytecodeCompiler → MetaData<BytecodeRefs>  (body_ref = BytecodeRef)
                                      │
                                      ▼
                            Runtime holds Arc<MetaData<BytecodeRefs>>
```

The runtime looks up compiled functions directly from `body_ref` in the metadata table. The
vtable mechanism is eventually replaced by metadata-level sig satisfaction queries. The checker's
query interface (`SymbolQuery`) becomes the foundation for an incremental query cache, which in
turn makes bidirectional elaboration and constraint-based checking tractable without restructuring
the whole pipeline each time.

Each stage below leaves the project in a state that compiles cleanly and all tests pass.

---

## Stage 1 — Arc-wrap `Trait` and `Signature` in `ast::Definition` `[not started]`

`Definition::Function`, `ExternalFunction`, `Struct`, and `TraitImpl` all carry Arc-wrapped named
structs. `Definition::Trait` and `Definition::Signature` are the remaining inline-fields variants.

Introduce `AstTrait` and `AstSignature` in `ast/mod.rs` and change both to tuple form, matching
the pattern established for `AstTraitImpl`. Update `CheckerAstRef::Trait` and
`CheckerAstRef::Signature` to hold `Arc<AstTrait>` and `Arc<AstSignature>` respectively.

Affected files: `ast/mod.rs`, `compiler/parser.rs`, `typecheck/checker.rs`,
`compiler/mod.rs`, `analysis/unused_return_values.rs`, parser tests.

Verification: all existing tests pass; `CheckerAstRef` has no unit variants carrying field copies.

---

## Stage 2 — `prelude` module for built-in types `[not started]`

Every name in the symbol table must belong to a module. `ModuleName::from_str("")` is used as a
sentinel in `seed_builtin_types`, `ast_type_to_type_name`, the bytecode compiler, the VM, and the
`il_analysis` test helpers. This is fragile and inconsistent with the invariant that everything
lives in a named module.

Two separate concerns are resolved here:

**Built-in types.** Rename `ModuleName::from_str("builtin")` to `ModuleName::from_str("prelude")`
throughout `seed_builtin_types` and `ast_type_to_type_name`. Strip type arguments from
parameterised built-in types so `List<Int>` maps to `TypeName { name: "List", module: "prelude" }`
and resolves against the seeded entry:

```
AstType::List(_)   => TypeName { name: "List",   module: ModuleName::from_str("prelude") }
AstType::Option(_) => TypeName { name: "Option", module: ModuleName::from_str("prelude") }
```

**Unqualified bytecode names.** The bytecode compiler emits `FunctionName` with
`module: ModuleName::from_str("")` for entry-module functions. These should use the actual module
name (`"main"` for the entry module, the declared name otherwise). The VM and the `il_analysis`
test helpers use `""` for the same reason; replace all of them with proper module names.

Add `ModuleName::UNQUALIFIED` as a compile-error marker (a deprecated constant) during the
transition so any remaining `from_str("")` sites are visible without a text search.

Verification: no `ModuleName::from_str("")` calls remain in production code; new tests assert
that `List<Int>` return type resolves against the `prelude::List` metadata entry.

---

## Stage 3 — Introduce `TypedRefs`; checker outputs `MetaData<TypedRefs>` `[not started]`

### Current passes in `check_modules`

There are three sequential passes today, distinct from any future constraint-solver step:

1. **Signature collection** — `collect_native_sigs`, `build_visibility_for_module`, and
   `collect_function_signatures` run for every module. This populates `MetaData<CheckerRefs>`
   with function signatures, struct types, trait definitions, impl definitions, and module entries.
   `CheckerAstRef` stores `Arc<ast::Function>` — untyped source nodes — because the typed ABT
   does not exist yet.

2. **Parameter sig registration** — `register_param_sigs` runs after all signatures are known so
   that module-parameter aliases resolve correctly against other modules' already-loaded functions.

3. **Expression checking** — `check_single_module_expressions` type-checks expression bodies and
   produces `typed_ast::Module` for each module. This is the step that elaborates the source AST
   into the typed ABT. It is not the HM(X) constraint-solver step described in
   `type-system-abstractions.md`; it is the existing single-pass HM unification walk.

### Two metadata tables

Rather than mutating already-inserted `Arc`-wrapped entries, the checker maintains two separate
tables:

- `MetaData<CheckerRefs>` — internal working table, `AstRef = Arc<ast::Function>`. Used throughout
  passes 1–3 for signature lookups, visibility queries, and trait resolution. Discarded after
  `check_modules` returns.

- `MetaData<TypedRefs>` — output table, `AstRef = Arc<typed_ast::Function>`. Populated during
  pass 3 as each function is elaborated. This is the value returned to the caller.

Define `TypedRefs` in `typecheck/checker.rs` alongside `CheckerRefs`. Only the two function-carrying
variants differ between the internal and output tables; everything else (`Struct`, `Trait`, `Impl`,
`Module`, `Signature`, `Builtin`, `ExternalFn`) carries the same data in both. `TypedCheckerAstRef`
wraps those unchanged variants via an `Other` catchall rather than duplicating them:

```
pub enum TypedCheckerAstRef {
    Function(Arc<typed_ast::Function>, FunctionKind),
    ImplFunction(Arc<typed_ast::Function>, String, FunctionKind),
    Other(CheckerAstRef),
}

pub struct TypedRefs;
impl References for TypedRefs {
    type Source  = SourceLocation;
    type Ast     = TypedCheckerAstRef;
    type Body    = NoBody;
    type Witness = NoWitness;
}
```

`check_modules` builds `MetaData<TypedRefs>` in parallel with the expression-checking pass,
inserting an entry for each elaborated function as it is produced. Non-function entries (structs,
traits, impls, modules) are copied from `MetaData<CheckerRefs>` after pass 3 completes, since
their shape does not change between the two tables.

`check_modules` returns `(function_kinds, MetaData<TypedRefs>)`, dropping the
`typed_modules: HashMap<String, typed_ast::Module>` return value. The compiler reconstructs
per-module structure from `metadata.modules` when needed.

Verification: all existing tests pass; no `Arc<ast::Function>` appears in the returned metadata;
the bytecode compiler accepts `MetaData<TypedRefs>` as input.

---

## Stage 4 — Visibility on `FunctionDefinition`; remove `ModuleVisibility` map `[not started]`

`ModuleVisibility` (`HashMap<String, bool>`) is built separately and threaded through `CheckContext`
and the compiler. `FunctionDefinition` has no visibility field.

Steps:

1. Add `pub visibility: Visibility` to `FunctionDefinition` in `symbols.rs`.
2. Populate it from `is_pub` in `collect_function_signatures` and `collect_native_sigs`.
3. Replace `module_visibility` lookups in `check_visibility` and `build_alias_to_qualified` with
   direct queries to `self.metadata.functions`.
4. Remove `ModuleVisibility` from `CheckContext` and `check_modules`.
5. Populate `ModuleDefinition.exports` from the public functions recorded in each module.
6. Set `ModuleDefinition.visibility` from `parsed.is_entry` and the module-level `is_pub` flag.
7. Derive `CompiledProgram::module_visibility` on demand from `metadata.functions` rather than
   maintaining it as a separate field.

Verification: the private-visibility error path fires correctly; `test_compile_project_two_files`
assertions on `module_visibility()` continue to pass.

---

## Stage 5 — `body_ref` populated by the bytecode compiler `[not started]`

Define `BytecodeRef` and a `BytecodeRefs` implementation of `References` in the bytecode module:

```
pub struct BytecodeRef {
    pub instructions: Vec<Instruction>,
    pub labels: HashMap<String, usize>,
}

pub struct BytecodeRefs;
impl References for BytecodeRefs {
    type Source = SourceLocation;
    type Ast    = CheckerAstRef;   // typed AST carried forward
    type Body   = BytecodeRef;
    type Witness = NoWitness;
}
```

The bytecode compiler takes `MetaData<CheckerRefs>` and produces `MetaData<BytecodeRefs>` by
iterating `metadata.functions`, compiling each function's body (read from the typed `ast_ref`),
and inserting a `BytecodeRef` as `body_ref`. This replaces the `CompiledFunction` HashMap in
`CompiledProgram` as the store of compiled code.

The runtime holds `Arc<MetaData<BytecodeRefs>>` and dispatches calls by looking up
`metadata.function(name)?.body_ref`.

`CompiledProgram` is simplified: it retains only the fields not yet covered by the metadata table
(entry point, source path, struct definitions, external function registry). These migrate in
subsequent stages as the metadata table gains the corresponding entry kinds.

Verification: all bytecode and VM tests pass with functions dispatched via `body_ref`.

---

## Stage 6 — Vtable replacement via metadata `[not started]`

The vtable mechanism resolves module parameters to concrete implementations at compile time and
rewrites call sites in `lower_typed_module`. In the metadata architecture, a module parameter
`(greeter: Greeter)` declares that the module requires any implementation satisfying the `Greeter`
sig. Binding a concrete module `greetlib` to that parameter is a `SymbolQuery::impl_for` lookup at
compile time, not a string-map rewrite at the expression level.

Steps:

1. Represent module parameter bindings in `MetaData` as `ImplDefinition` entries linking the
   concrete module to the sig it satisfies, keyed by `ImplKey { type_name, trait_name }`.
2. `resolve_vtables` becomes a metadata query: for each module parameter, find the `ImplDefinition`
   that satisfies the declared sig and record the concrete module name.
3. `lower_typed_module` reads dispatch targets from `ImplDefinition.ast_ref` rather than from a
   separate string map.
4. Delete `Vtables`, `resolve_vtables`, and the vtable-rewriting pass once all dispatch targets
   are resolved via the metadata query.

The sig satisfaction check that currently happens implicitly (functions enumerated and matched by
name) becomes an explicit `SymbolQuery::impl_for` assertion during compilation, with a proper error
if no satisfying implementation is found.

Verification: the four vtable tests in `compiler/mod.rs` and `compiler/wiring.rs` continue to pass.

---

## Stage 7 — Checker reads go through `SymbolQuery` `[not started]`

The checker's internal helpers (`get_function_sig`, `get_struct_fields`, `get_trait_functions`,
`get_sig_functions`) access `self.metadata.functions` and `self.metadata.types` directly. This
couples the checker to the concrete `MetaData` struct.

Change all helpers to call through `SymbolQuery` methods. Where the trait lacks a needed query
(e.g. finding a struct by unqualified name), add a new default method to `SymbolQuery` with a
blanket implementation on `MetaData<R>`.

This is the prerequisite for a query cache: once all reads go through the trait, an intercepting
implementation memoises results without modifying a single call site. It is also the precondition
for bidirectional elaboration, where the "check" and "synth" modes both issue queries rather than
walking internal maps.

Verification: no direct field access to `self.metadata.functions` or `self.metadata.types` remains
in `checker.rs`; all tests pass.

---

## Stage 8 — Generic type parameter validation `[not started]`

`Generic(String)` in `AstType` represents a type-parameter reference. No validation checks that
the name is bound by a `type_params` declaration on the enclosing function or struct. An unbound
generic silently produces an unknown type at elaboration time.

In `collect_function_signatures`, after resolving each function's parameter and return types, check
that every `Generic(name)` in those types appears in `func.type_params`. Emit a descriptive
`TypeError` if not.

Add tests covering: correctly-bound generics, an unbound generic name producing an error, and a
generic struct field.

---

## Toward the query-based architecture

The design in `type-system-abstractions.md` describes bidirectional elaboration sitting atop
constraint emission and a query cache. The stages above are sequenced to reach that target without
a rewrite:

- Stage 3 (typed AST in metadata) is the prerequisite for elaboration output being meaningful.
- Stage 7 (`SymbolQuery` as the read interface) is the prerequisite for a cache: add a memoising
  wrapper around `SymbolQuery` and every existing call site benefits.
- The split between signature collection (pass 1) and expression checking (pass 3) already mirrors
  the separation between constraint generation and solving. Extracting unification into a
  `Constraints` struct in a future stage will not require restructuring the pass layout.
- HKTs require kind checking as a separate concern. The `References::Ast` associated type is the
  natural place to attach a kind annotation to each `TypeDefinition` once kind inference is added,
  without changing the table structure.