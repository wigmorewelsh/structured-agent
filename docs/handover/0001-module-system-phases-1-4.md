# Module System Phases 1–4

## Original Goal

Implement the first four phases of the module system described in [module-system-implementation.md](../module-system-implementation.md). Each phase was to deliver usable value independently, with all existing tests passing after each increment.

The four phases were:

- Phase 1: `pub` visibility parsed and stored, `is_pub` on AST nodes
- Phase 2: `use` aliases parsed, stored in the AST, and resolved by the type checker within a single file
- Phase 3: `mod` header parsing and demand-driven multi-file loading via `Compiler::compile_file`
- Phase 4: `sig` declarations parsed and stored; cross-module visibility enforced by the type checker

## What Was Done

### AST (`src/structured-agent/src/ast/mod.rs`)

`is_pub: bool` was added to both `Function` and `ExternalFunction`. The field defaults to `false` at all existing construction sites. `Display` impls print `pub ` when set.

Three new `Definition` variants were added:

```rust
Definition::Use {
    path: Vec<String>,
    alias: Option<String>,
    is_pub: bool,
    span: Span,
}

Definition::ModuleHeader {
    name: String,
    params: Vec<ModuleParam>,
    span: Span,
}

Definition::Signature {
    name: String,
    functions: Vec<SigFunction>,
    span: Span,
}
```

`ModuleParam` carries a local parameter name and a dotted module path. `SigFunction` carries a name, parameter list, and return type without a body.

### Parser (`src/structured-agent/src/compiler/parser.rs`)

`parse_function` and `parse_external_function` each accept an optional leading `pub` keyword via `optional(attempt(lex_string("pub")))`.

`parse_use` reads `pub? use dotted.path (as alias)?`. The dotted path is parsed as identifiers separated by `.` with no surrounding whitespace. It is added to the `choice` in `parse_program` under `attempt`.

`parse_module_header` reads `mod name (params)?` where params is a comma-separated list of `name: dotted.path` inside parentheses. It is tried as `optional(attempt(...))` before the main `many(choice(...))` loop.

`parse_sig_definition` reads a `sig Name { ... }` block containing zero or more bare function signatures of the form `fn name(params): ReturnType`. It is added to the main `choice`.

### Type Checker (`src/structured-agent/src/typecheck/checker.rs`)

`CheckContext` was introduced as a short-lived struct carrying the four values previously threaded as separate arguments through every recursive call:

```rust
struct CheckContext<'a> {
    file_id: FileId,
    alias_map: &'a HashMap<String, String>,
    module_visibility: &'a ModuleVisibility,
    alias_to_qualified: &'a AliasToQualified,
}
```

`check_module` delegates to `check_module_with_external_sigs` with empty maps, giving a single entry point. The method accepts `external_sigs` and `module_visibility`.

`build_alias_to_qualified` builds a map from local alias to fully-qualified name by cross-referencing `Definition::Use` entries against `module_visibility`. This map is consulted in `check_visibility` to detect private cross-module calls even when the call site uses an alias.

`FunctionSignatureTuple` is a public type alias for `(Vec<Parameter>, AstType, bool)`. `ModuleVisibility` and `AliasToQualified` are similarly aliased.

Expression checking was split into named helpers — `check_call`, `check_visibility`, `check_boolean_condition`, `check_block`, `check_list_literal`, `check_select`, `check_if_else_expression`, `check_struct_literal`, `check_field_access` — each receiving a `&CheckContext`.

### Compiler (`src/structured-agent/src/compiler/mod.rs`)

The compiler was subsequently refactored into a pipeline of discrete phases with clear inputs and outputs. The old entangled `discover_and_parse` / `compile_module` approach was replaced. The current structure is:

**`compiler/discovery.rs`** — The `Discoverer` trait abstracts source resolution:

```rust
pub(crate) trait Discoverer {
    fn resolve(&self, path: &str) -> Result<String, String>;
    fn dep_path(&self, entry_dir: &str, module_name: &str) -> String;
}
```

`FileDiscoverer` reads from disk. `InMemoryDiscoverer` looks up from a `HashMap<String, String>`. The `discover` function performs BFS over module references, parsing each file once to extract dependencies. The parsed `Module` is carried forward on `ParsedModule` — there is no second parse.

`ParsedModule` is the output of discovery:

```rust
pub(crate) struct ParsedModule {
    pub(crate) name: String,
    pub(crate) module: Module,
    pub(crate) is_entry: bool,
    pub(crate) file_id: FileId,
}
```

**`compiler/sigs.rs`** — `SigTable` is the output of the sig-collection phase:

```rust
pub(crate) struct SigTable {
    pub(crate) visibility: ModuleVisibility,
    pub(crate) external_sigs: HashMap<String, FunctionSignatureTuple>,
}
```

`collect_sigs` folds over `&[ParsedModule]` to produce a `SigTable`. `sigs_visible_to_module` filters the table to only the sigs imported by a given module's `use` declarations.

**`compiler/mod.rs`** — The pipeline phases are free functions:

- `type_check_module(parsed, sig_table) -> Result<(), TypeError>` — pure, no reporter. Returns the error as data.
- `analyse_module(parsed) -> Vec<Warning>` — pure, no reporter. Returns warnings as data.
- `emit_module(module, prefix) -> Result<ModuleArtifact, String>` — produces a `ModuleArtifact` per module without mutating shared state.

`ModuleArtifact` holds the four collections a single module produces:

```rust
struct ModuleArtifact {
    functions: Vec<Box<dyn ExecutableFunction>>,
    external_functions: Vec<ExternalFunctionDefinition>,
    struct_definitions: Vec<(String, Vec<(String, Type)>)>,
    sig_definitions: Vec<(String, Vec<SigFunction>)>,
}
```

`CompiledProgram::merge` folds a `ModuleArtifact` into the program. The `add_*` mutation methods were removed; `merge` is the only mutation path. `module_visibility` is private; the accessor `module_visibility()` is the public surface.

The orchestration in `compile` runs three sequential loops: check all modules, then emit all modules, then merge artifacts. Reporter interaction (emitting diagnostics) is confined to the orchestrator and does not leak into the phase functions.

`compile_source` and `compile_file` are thin public wrappers that construct the appropriate `Discoverer` and delegate to the private `compile` method:

```rust
fn compile(
    &self,
    entry_path: &str,
    entry_source: &str,
    source_path: Option<String>,
    discoverer: &impl Discoverer,
) -> Result<CompiledProgram, String>
```

### Runtime (`src/structured-agent/src/runtime/engine.rs`)

`RuntimeBuilder` and `Runtime` hold a `ProgramSource` directly. The runtime calls `Compiler::compile_source` or `Compiler::compile_file` at `run()` and `check()` time via a private `compile()` method. `load_program` and `builder_from_compiled` no longer exist.

### CLI (`src/structured-agent/src/cli/app.rs`)

`App` imports only `crate::runtime::Runtime`. The compiler is not imported by the CLI. Whether the source is a file or inline string is decided inside the runtime.

### ACP Agent (`src/structured-agent/src/acp/agent.rs`)

`Agent` holds a single `ProgramSource` field. The agent passes it directly to `Runtime::builder`.

## Layer Diagram

```
cli/app.rs
    |
    | ProgramSource
    v
runtime/engine.rs  (Runtime::builder, run, check)
    |
    | compile_source / compile_file
    v
compiler/mod.rs  (Compiler::compile)
    |
    +-- discovery.rs     discover() -> Vec<ParsedModule>
    +-- sigs.rs          collect_sigs() -> SigTable
    +-- type_check_module() -> Result<(), TypeError>
    +-- analyse_module()    -> Vec<Warning>
    +-- emit_module()       -> ModuleArtifact
    +-- CompiledProgram::merge()
```

Nothing above `runtime` touches the compiler directly. Nothing in the compiler touches the runtime. Each phase function is a pure function of its inputs.

## What Is Not Working or Not Yet Done

### Phase 4 sig satisfaction is parse-only

`sig` definitions are parsed and stored in `CompiledProgram::sig_definitions`. The type checker does not yet verify that a module declaring `mod name: SomeSig` satisfies the sig.

### `pub use` re-export semantics are not enforced

`pub use` is parsed and the `is_pub` field is set on the `Definition::Use` node, but the type checker makes no distinction between `use` and `pub use` at cross-module call sites.

### `mod name: Sig` on a ModuleHeader is not parsed

The grammar for `mod name: Sig` was noted in the implementation plan but not added to the parser. `ModuleHeader` carries a `params` field but no `sig` assertion field.

### Phases 5 and 6 are not started

Module parameters, compile-time wiring, and structural contract matching remain unimplemented.

## Known Cleanup Opportunities

`CompilationUnit` is used only internally by `Compiler`. It could be collapsed into the parser call directly, removing the type from the public API.

`FunctionSignatureTuple` is a three-tuple `(Vec<Parameter>, AstType, bool)`. The `bool` is `is_pub`. This would be cleaner as a named struct.

The reporter interaction in the orchestrator loop — cloning the reporter and emitting warnings imperatively — could become a `report_diagnostics(warnings, errors, reporter)` helper, fully decoupling the phase functions from diagnostic emission.

## See Also

- [module-system-implementation.md](../module-system-implementation.md)
- [module-system.md](../module-system.md)
- [type-theory-llm-guardrails.md](../type-theory-llm-guardrails.md)
- `src/structured-agent/src/compiler/mod.rs`
- `src/structured-agent/src/compiler/discovery.rs`
- `src/structured-agent/src/compiler/sigs.rs`
- `src/structured-agent/src/typecheck/checker.rs`
- `src/structured-agent/src/ast/mod.rs`
- `src/structured-agent/src/runtime/engine.rs`
