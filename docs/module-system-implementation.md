# Module System Implementation

The module system described in [module-system.md](module-system.md) requires changes across every layer of the compiler and runtime. This document maps each feature to the code it touches and breaks the work into phases that each add usable value independently.

Phases 1 through 4 are complete. The architecture has been substantially refactored since this document was first written. The current compiler pipeline is described below.

## Current Architecture

The compiler is organised into discrete phases, each a pure function of its inputs. Discovery, sig collection, type checking, analysis, and emission are separated.

```
Discoverer (FileDiscoverer or InMemoryDiscoverer)
    → discover() → Vec<ParsedModule>
    → collect_sigs() → SigTable
    → type_check_module() → Result<(), TypeError>      (per module, pure)
    → analyse_module() → Vec<Warning>                  (per module, pure)
    → emit_module() → ModuleArtifact                   (per module, pure)
    → CompiledProgram::merge()
    → Runtime
```

The `Discoverer` trait abstracts source resolution, with `FileDiscoverer` (reads from disk) and `InMemoryDiscoverer` (reads from a `HashMap`) as the two implementations. Discovery performs BFS over module references, parsing each file once and carrying the result forward — there is no second parse.

`SigTable` holds the full cross-module visibility map and external signatures, produced by a single fold over all `ParsedModule`s before any type checking begins.

Each phase function takes its specific inputs and returns data. No phase function holds a reporter or writes diagnostics directly; that is the orchestrator's responsibility. The orchestrator runs all type checks, then all emissions, then merges artifacts into a `CompiledProgram`.

`compile_source` and `compile_file` are thin public wrappers that construct the appropriate `Discoverer` and delegate to the private `compile` method.

Function names remain flat strings with dots as separators (e.g. `greetlib.greet`). The runtime splits on `.` at lookup time.

## Chosen Approaches

Before the phases, the non-obvious choices made for each area:

**File discovery (Area 1)** — demand-driven loading. The compiler parses the entry file, inspects its `mod` and `use` declarations, and loads referenced files on demand. Only files reachable from the entry point are loaded. Requires `mod` declarations to be parsed before full compilation of dependents can proceed.

**Qualified names (Area 4)** — keep `function: String` in `Expression::Call`, allow `::` in the string. The parser produces `Call { function: "greetlib::greet" }` for `greetlib::greet(...)`. This avoids touching every match on `Expression::Call` in the type checker, analysis passes, and bytecode compiler until the module system is proven out. Can be replaced with a structured `Vec<String>` later. `::` is used instead of `.` throughout to unambiguously distinguish module paths from struct field access, which retains `.`.

**Visibility enforcement (Area 6)** — type checker only. The runtime trusts the type checker, consistent with how all other type safety is handled today.

**Module parameter wiring (Area 7)** — vtable dispatch at runtime. Each parameterised module carries a vtable: a map from parameter-qualified names (e.g. `io.read`) to concrete names (e.g. `storage.disk.read`), populated at compile time from wiring declarations and stored in `CompiledProgram`. The bytecode emits `Call { function_name: "io.read" }` unchanged. The VM resolves through the vtable at dispatch. The type checker validates calls against the parameter's declared sig (named or derived from a module's public exports) without needing to know the concrete wiring. Diagnostics always refer to the original source names.

## Phase 1: `pub` Visibility Within a Single File — DONE

**Value delivered:** The parser accepts `pub fn` and the type checker enforces that non-`pub` functions are not callable from outside their declared module. The visibility foundation for multi-file work.

**Files changed:**

`ast/mod.rs` — Add `is_pub: bool` to `Function` and `ExternalFunction`.

`compiler/parser.rs` — `parse_function` and `parse_external_function` handle an optional leading `pub` keyword. All existing functions without `pub` parse as `is_pub: false`. Module paths in `use` and `mod` declarations use `::` as separator (`use greetlib::greet`). Call expressions also parse `foo::bar(args)` as `Call { function: "foo::bar" }`, making module calls syntactically distinct from field access (which retains `.`).

`typecheck/checker.rs` — `collect_function_signatures` stores `is_pub` alongside each signature. Cross-file visibility enforcement is via `check_visibility`, which consults `module_visibility` in `CheckContext`.

## Phase 2: `use` Aliases Within a Single File — DONE

**Value delivered:** `use` and `pub use` parse and are recorded in the AST. `use math.add as add` creates a local alias resolved by the type checker. `pub use` is parsed but re-export semantics are not yet enforced.

**Files changed:**

`ast/mod.rs` — New `Definition::Use` variant:

```
Definition::Use {
    path: Vec<String>,
    alias: Option<String>,
    is_pub: bool,
    span: Span,
}
```

`compiler/parser.rs` — `parse_use` reads `pub? use path::segments (as alias)?` using `::` as the path separator. `parse_call` uses `sep_by1(identifier_raw(), "::")` so that `io::read(args)` produces `Call { function: "io::read" }`. Field access on struct values retains `.` and remains syntactically distinct.

`typecheck/checker.rs` — Before checking function bodies, build a local alias map from `Definition::Use` entries. Calls that match an alias are resolved through it before the standard signature lookup.

`compiler/mod.rs` — `Definition::Use` entries are skipped during bytecode emission.

## Phase 3: `mod` Header and Demand-Driven File Loading — DONE

**Value delivered:** Files can declare `mod name` at their head. The compiler follows `mod` and `use` declarations to load additional files on demand. Functions from loaded modules are registered under qualified names (`module.function`). Bare names within the entry module continue to work.

**Files changed:**

`ast/mod.rs` — New `Definition::ModuleHeader` variant:

```
Definition::ModuleHeader {
    name: String,
    params: Vec<ModuleParam>,
    span: Span,
}
```

`ModuleParam` holds a local parameter name and a `::`-separated path to a module or sig (e.g. `io: storage::Storage`).

`compiler/parser.rs` — `optional(parse_module_header())` is attempted before the `many(choice(...))` loop. `parse_module_header` reads `mod name (params)?`. Module param paths use `::` as separator.

`compiler/discovery.rs` — The `Discoverer` trait with `FileDiscoverer` and `InMemoryDiscoverer` implementations. The `discover` function performs BFS, parsing each file once to extract `Use` and `ModuleHeader` references. `ParsedModule` carries the parsed `Module`, `FileId`, module name, and `is_entry` flag.

`compiler/sigs.rs` — `SigTable` collects visibility and external signatures from all `ParsedModule`s in a single pass. `sigs_visible_to_module` filters to only the sigs a given module has imported.

`compiler/mod.rs` — `compile_source` constructs an `InMemoryDiscoverer`; `compile_file` constructs a `FileDiscoverer`. Both delegate to `compile`. The `compile` method runs discovery, sig collection, per-module type checking and analysis, per-module emission, and merges artifacts.

`runtime/engine.rs` — `RuntimeBuilder` holds a `ProgramSource` and calls `compile_source` or `compile_file` at runtime startup.

## Phase 4: `sig` Declarations and Cross-Module Visibility — DONE

**Value delivered:** `sig` blocks parse and are stored. The type checker enforces that cross-module calls respect `pub` visibility.

**Not yet complete:** The type checker does not verify that a module declaring `mod name: SomeSig` actually satisfies the sig. `pub use` re-export semantics are parsed but not enforced. `mod name: Sig` assertion syntax is not yet in the parser.

**Files changed:**

`ast/mod.rs` — New `Definition::Signature` variant:

```
Definition::Signature {
    name: String,
    functions: Vec<SigFunction>,
    span: Span,
}
```

`SigFunction` is a name and type signature without a body.

`compiler/parser.rs` — `parse_sig_definition` added to the `choice` in `parse_program`.

`compiler/mod.rs` — `CompiledProgram` gains `sig_definitions: HashMap<String, Vec<SigFunction>>`. The compiler pass over `Definition::Signature` populates it.

`typecheck/checker.rs` — `check_visibility` enforces `pub` on cross-module calls using `module_visibility` from `SigTable`. Cross-module calls are identified by the presence of `::` in the resolved function name. The two-pass requirement (collect all sigs, then verify) is satisfied by `collect_sigs` running over all `ParsedModule`s before any `type_check_module` call.

`compiler/sigs.rs` — `collect_sigs` and `sigs_visible_to_module` implement the two-pass collection.

**Not in this phase:** Module parameters and dependency wiring. Sig satisfaction checking and `pub use` re-export enforcement remain to be implemented.

**Separator note:** All qualified names throughout the compiler and runtime use `::` as the module path separator (e.g. `greetlib::greet`). Struct field access retains `.`. This applies to `ModuleVisibility` keys, `external_sigs` keys, vtable keys, and emitted function names.

## Phase 5: Module Parameters and Vtable Dispatch — DONE

**Value delivered:** `mod db(io: storage::Storage)` declares a module parameter. The type checker validates calls to `io::read()` against the `Storage` sig without knowing the concrete module. The compiler builds a vtable mapping `io::read` → `storage::read` and stores it in `CompiledProgram`. The VM resolves through the vtable at call dispatch using the calling function's `module_name`. Diagnostics always refer to original source names — no AST rewriting.

**Pipeline changes:**

Phase 5 introduces one new stage between `collect_sigs` and `type_check_module`, and populates vtables during emission:

```
collect_sigs() → SigTable                                (sig_definitions added)
    → type_check_module()                                (param calls checked against sig)
    → analyse_module()
    → emit_module() → ModuleArtifact                     (unchanged)
    → resolve_vtables(modules, sig_table) → Vtables      (new, after all emission)
    → CompiledProgram::merge() + set_vtables()
    → Runtime / VM resolves param calls through vtables
```

`SigTable` carries sig definitions alongside visibility and external sigs:

```rust
pub(crate) struct SigTable {
    pub(crate) visibility: ModuleVisibility,
    pub(crate) external_sigs: HashMap<String, FunctionSignatureTuple>,
    pub(crate) sig_definitions: HashMap<String, Vec<SigFunction>>,
}
```

`collect_sigs` populates `sig_definitions` from `Definition::Signature` nodes across all parsed modules. This is already done in Phase 4's `sigs.rs` changes.

`typecheck/checker.rs` — `register_param_sigs` is called before `collect_function_signatures`. For each `ModuleHeader` param, it looks up the named sig in `sig_definitions` (or derives an implicit sig from that module's public exports in `external_sigs` if no named sig is declared). The sig's functions are registered under `param::fn` keys (e.g. `io::read`), giving the type checker everything it needs to validate calls like `io::read()`. No wiring knowledge is required.

`compiler/wiring.rs` — `resolve_vtables(modules, sig_table) -> Vtables` folds over all parsed modules, finds those with `ModuleHeader` params, and for each param builds vtable entries mapping `param::fn` → `concrete::fn` for every function in the sig (named or derived). `Vtables` is `HashMap<String, HashMap<String, String>>` — module name → (`param::fn` → `concrete::fn`).

`CompiledProgram` — gains `vtables: Vtables`. `CompiledFunction` (in `bytecode/compiler.rs`) gains a `module_name: Option<String>` field, set by `emit_module` when a non-entry prefix is present.

`bytecode/vm.rs` — `execute_call` now takes `module_name: Option<&str>` from the executing `CompiledFunction`. Before the registry lookup, it checks `runtime.vtables().get(module_name)` and substitutes the concrete name if `function_name` is a key in that vtable. The bytecode instruction `Call { function_name: "io::read" }` is emitted unchanged by the bytecode compiler.

`ast/mod.rs` — No changes required. `Definition::ModuleHeader` already carries `params: Vec<ModuleParam>` from Phase 3.

**Done:** `::` separator language-wide. `SigTable::sig_definitions`. `register_param_sigs` in the type checker. `resolve_vtables` in `compiler/wiring.rs`. `CompiledProgram::vtables`. `CompiledFunction::module_name`. Vtable dispatch in `VM::execute_call`. `use` alias runtime registration. End-to-end test and samples passing.

**Not in this phase:** Structural contract matching against a concrete module.

## Phase 5a: Wiring Site Syntax — DONE

**What was implemented:**

Two new AST nodes and their parsers, plus updated vtable resolution to use them.

`Definition::ModuleBinding { name, sig_path, impl_path, span }` — represents `mod fmt: formatter::Formatter = formatter` at the wiring site. Binds a name to a concrete module and asserts it satisfies the named sig.

`Definition::WiringSite { name, args, span }` — represents `mod reporter(fmt)` at the wiring site. Passes a pre-bound name into a parameterised file module by position, matching each arg to the corresponding param in the parameterised module's `ModuleHeader`.

**Parser (`compiler/parser.rs`)**

`parse_module_binding` parses `mod name: sig::Path = impl::path`. Tried before `parse_module_header` in both the file header slot and the definition list.

`parse_wiring_site` parses `mod name(bare_ident, ...)` — bare identifiers only inside `(...)`, no `:`. Placed before `parse_module_header` in the definition list. `attempt` ensures clean backtracking if the content has a `:`.

**Discovery (`compiler/discovery.rs`)**

`referenced_module_names` follows `impl_path[0]` on `ModuleBinding` so demand-driven loading picks up the concrete module.

**Wiring (`compiler/wiring.rs`)**

`collect_bindings` gathers all `ModuleBinding` nodes — `name → impl_path[0]`.

`collect_wiring_sites` gathers all `WiringSite` nodes — `module_name → [bound_name, ...]`.

`resolve_vtables` now: for each parameterised module, checks whether a `WiringSite` names it; if so, resolves each param's concrete module via the corresponding wiring site arg looked up in `collect_bindings`. Falls back to implicit inference (`param.path[0]`) for modules with no wiring site, preserving backward compatibility with existing tests.

**Samples (`samples/modules/wired.sa`)**

```
mod fmt: formatter::Formatter = formatter
mod reporter(fmt)

use reporter::report

fn main(): () {
    "Demonstrating vtable dispatch via module parameters"!
    report("hello from wired modules")
    "Done"!
}
```

The binding `mod fmt` names `formatter` as the concrete module. `mod reporter(fmt)` passes that binding into `reporter`, whose vtable is built from the binding rather than from inference. Substitution is demonstrated in `test_vtable_substitution_end_to_end` — `sub_mock` is wired in place of `sub_real` and the VM dispatches to the mock.

**Not implemented:** Satisfaction checking — verifying that the concrete module named in `impl_path` actually exports all functions required by `sig_path`. This is deferred to Phase 6.

## Phase 6: Module Contract Matching — NOT STARTED

**Value delivered:** `mod mock: storage.disk` declares that a module satisfies the same contract as a concrete module, without requiring a named sig. The compiler derives the required surface from `storage.disk`'s public exports and verifies the declaring module matches it structurally. This is the primary mechanism for test doubles.

**Pipeline changes:**

Phase 6 adds a new pure phase function consistent with the pattern established in phases 1–4:

```
collect_sigs() → SigTable
    → check_sig_satisfaction(module, sig_table) → Result<(), TypeError>   (new)
    → resolve_wiring() → WiringMap
    → ...
```

`check_sig_satisfaction` takes a `ParsedModule` and the `SigTable` and verifies that every `pub fn` in the referenced module or named sig is present in the declaring module with a matching signature. It returns `Result<(), TypeError>` with no reporter, consistent with `type_check_module`. The orchestrator handles reporting.

The distinction between a sig reference and a module reference in `mod name: X` is resolved by checking whether `X` appears in `SigTable::sig_definitions` (a named sig) or in `SigTable::visibility` (a module path). Both cases are handled by `check_sig_satisfaction` — the structural surface is derived from whichever source applies.

`compiler/mod.rs` — The orchestrator loop gains a `check_sig_satisfaction` call after `collect_sigs` and before `resolve_wiring`. No changes to the type checker's call-checking logic are needed; this is an additional verification pass over module declarations, not call sites.

## Area 10: Analysis Passes

The analysis passes in `analysis/` all operate on a single `ast::Module` and are unaffected by the phases above. During `compile_project` each module is run through the analysis passes independently. Cross-project analysis (e.g. detecting unused `pub fn` across all modules) is out of scope for all phases above and can be addressed separately.

## Summary of Changes by File

| File | Phase |
|---|---|
| `ast/mod.rs` | 1: `is_pub` on functions; 2: `Use`; 3: `ModuleHeader`; 4: `Signature`; 5: `Display` uses `::` for `Use` and `ModuleHeader` paths; 5a: `ModuleBinding`, `WiringSite` |
| `compiler/parser.rs` | 1: `pub` keyword; 2: `use`; 3: `mod` header; 4: `sig`; 5: `::` separator in `parse_use`, `parse_module_param`, `parse_call`; 5a: `parse_module_binding`, `parse_wiring_site` |
| `compiler/mod.rs` | 3: demand-driven loading via `Discoverer`; 4: sig map, two-pass collection; 5: `vtables` on `CompiledProgram`, `resolve_vtables` call; 6: `check_sig_satisfaction` stage |
| `compiler/discovery.rs` | 3: `Discoverer` trait, `FileDiscoverer`, `InMemoryDiscoverer`; 5a: follows `impl_path` on `ModuleBinding` |
| `compiler/sigs.rs` | 3: `SigTable`, `collect_sigs`; 4/5: `sig_definitions` on `SigTable` |
| `compiler/wiring.rs` | 5: `resolve_vtables` → `Vtables`; 5a: `collect_bindings`, `collect_wiring_sites`, explicit dispatch via wiring site |
| `typecheck/checker.rs` | 1: store `is_pub`; 2: alias resolution; 4: cross-module visibility via `::` check; 5: `register_param_sigs` for param call resolution; 6: structural surface derivation in `check_sig_satisfaction` |
| `bytecode/compiler.rs` | 5: `module_name: Option<String>` on `CompiledFunction` |
| `bytecode/vm.rs` | 5: vtable resolution in `execute_call` via `module_name` |
| `runtime/engine.rs` | 3: `RuntimeBuilder` holds `ProgramSource`, delegates to `Compiler`; 5: `vtables` field populated from `CompiledProgram` after compilation |
| `samples/modules/wired.sa` | 5a: uses `mod fmt: formatter::Formatter = formatter` and `mod reporter(fmt)` |

## See Also

- [module-system.md](module-system.md) — the full design
- [docs/type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — CMTT and the `in M` constraint, which builds on the module parameter mechanism in Phase 5
