# Module System

SA's module system serves two purposes that most languages treat separately: organising code across files and bounding what LLM-generated code may call at runtime. The two are unified by the same mechanism. A module is a named scope. A signature is the optional type of a module. The `in M` constraint on a generated function is the same thing as a module parameter — the generated code is handed a module the way a parameterised module is handed a dependency.

The design draws primarily from Rust's explicit module tree, Standard ML's signature/structure distinction, and the 1ML unification of modules and values. The dependency injection pattern is influenced by Newspeak's principle that nothing is globally reachable; everything is handed in. The formal grounding for the LLM guardrail use is Contextual Modal Type Theory (CMTT), developed by Nanevski, Pfenning, and Pientka.

## File as Module

Every `.sa` file is a module. Its identity is its path relative to the project root, with slashes replaced by dots and the `.sa` extension dropped. A file at `storage/disk.sa` is the module `storage.disk`. No declaration in a parent file is required.

Module headers — the `mod` declaration at the top of a file, including any parameter and sig references — always use fully qualified names rooted at the project root. This is true regardless of where the file lives in the directory tree. `use` inside the body may refer to modules by shorter names, but the header is always absolute. This means a search for `utils.Utils` across the project finds every module that depends on it.

Given a layout such as:

```/dev/null/layout.txt#L1-7
root/
  main.sa
  math.sa
  utils.sa
  tasks/
    cleaner.sa
    analyzer.sa
```

`main.sa` can use any module by its fully qualified name:

```/dev/null/main.sa#L1-4
mod main

use math
use tasks.cleaner
```

`tasks/cleaner.sa` uses shortened `use` for siblings in the same directory, but its module header — including any parameter types — always uses fully qualified names:

```/dev/null/tasks/cleaner.sa#L1-4
mod cleaner(utils: utils.Utils)

use analyzer
```

`utils.Utils` is the fully qualified name of the `Utils` sig defined in `utils.sa`, regardless of where `cleaner.sa` lives. `use analyzer` in the body is shortened because `analyzer.sa` shares the `tasks/` directory — the compiler resolves it to `tasks.analyzer`. Shortened `use` paths are valid only within the body of a file, never in a module header, and only when the short name is unambiguous within the project tree.

If `cleaner` needs `utils` passed in, `main.sa` wires the dependency using `mod`, not `use`:

```/dev/null/main.sa#L1-5
mod main
mod tasks.utils

mod tasks.cleaner(utils)

use math
```

`mod tasks.cleaner(utils)` passes the `utils` module into `cleaner` at the wiring site. `use` remains purely an alias mechanism and does not wire dependencies.

```structured-agent/src/structured-agent/samples/hello.sa#L1-4
fn main(): () {
    "Hello, World!"!
    "This is a simple hello world program"!
}
```

The module name `hello` is derived from the file path. `main` is not marked `pub` and is not named in any signature, so it is private to the module — the entry point convention rather than a callable export. The module tree is populated by the filesystem; there is no separate registry. The fully qualified name of `main` within the project is `hello.main`, though nothing outside the module can call it.

## Inline Modules

A module can also be declared inline within a file using a `mod` block. The inline and file forms are equivalent and composable — the module tree does not distinguish between them.

```/dev/null/example.sa#L1-5
mod math {
    pub fn add(a: Int, b: Int): Int { ... }
    fn internal(): Int { ... }
}
```

This occupies the same namespace as a file `math.sa` would. Inline modules are useful for small, tightly scoped definitions that do not warrant a separate file, and for test doubles defined alongside the test.

## Visibility

Three visibility levels exist, determined by `pub` and sig membership rather than by a separate access control keyword.

A function marked `pub` is accessible to any code that holds the module directly — via a qualified name or a `use` alias. This is the ordinary case for shared utilities and public APIs.

A function not marked `pub` but named in a signature the module satisfies is accessible to code holding the sig type, not to code holding the module directly. The sig is the lens; the function need not advertise itself beyond that contract. This means a module can expose different surfaces to different callers by satisfying multiple signatures, each naming a different subset of its functions.

A function marked neither `pub` nor named in any sig is private to the module. No external code can reach it regardless of how it holds the module.

These two axes are independent. A `pub fn` named in a sig is accessible both ways. A `pub fn` not in any sig is accessible only via the module directly. A non-`pub` fn in a sig is accessible only via the sig. The compiler enforces all three cases statically.

## Signatures

Signatures are optional. Most modules will not declare one. They exist for three specific purposes: abstracting over implementations in module parameters, bounding what LLM-generated code may call via `in M`, and presenting a stable public API through re-exports. A module without a signature is perfectly valid — its public surface is simply everything marked `pub`.

A signature is a named collection of function types declared with the `sig` keyword. It lives in the module tree like any other definition.

```/dev/null/storage/sig.sa#L1-5
sig Storage {
    fn read(key: String): Option<String>
    fn write(key: String, value: String): ()
}
```

This signature is reachable as `storage.Storage`. A module asserts it satisfies a signature at its declaration site:

```/dev/null/storage/disk.sa#L1-6
mod disk: storage.Storage

fn read(key: String): Option<String> { ... }
fn write(key: String, value: String): () { ... }
```

The compiler verifies that every function named in the signature is present and has a matching type. `read` and `write` are not marked `pub` — they are accessible only through the `Storage` sig, not via `storage.disk` directly. Functions in the module beyond those named in the signature are not visible to code that holds only the signature type. This is opaque ascription in the sense of Standard ML's `:>` operator — the internal representation is hidden from callers that interact via the signature.

Signatures have no special location requirement. One used across many modules is naturally placed in a shared file. One used only to bound an LLM context can be declared inline in the same file as the generation call.

## use and Re-export

`use` is an alias. It does not change what is in scope — fully qualified names always work — but it reduces repetition at use sites.

```/dev/null/agent.sa#L1-4
use storage::read as load_raw

fn load(key: String): Option<String> { load_raw(key) }
```

`use M::f as g` binds `g` as a local alias for `M::f`. `use M::f` without an alias brings `f` into scope unqualified. The qualified form is preferred where the origin aids readability. `use` paths inside a file body may be shortened — `use analyzer::thing` rather than `use tasks::analyzer::thing` when both files share the `tasks` directory — but this is purely a convenience. The compiler resolves them against the full module tree.

Adding `pub` to a `use` declaration re-exports the name as part of the current module's public surface. This is the mechanism for presenting a curated API that hides internal structure — equivalent to Rust's `pub use` or a TypeScript barrel file.

```/dev/null/storage.sa#L1-4
pub use disk::read
pub use disk::write as write_through
```

Callers of `storage` see only `read` and `write_through`. The `disk` and `memory` submodules are not visible — they are an implementation detail. A caller doing `use storage::read` has no knowledge of which submodule satisfies the call.

This composes with signatures. If `mod storage: StorageApi`, then `pub use` is the mechanism by which the names named in `StorageApi` are brought to the module's surface from wherever they actually live internally. The signature asserts the shape; `pub use` constructs it from the pieces.

## Module Parameters

A module can declare dependencies as parameters. This is opt-in and requires a signature on the parameter — since the module receiving the dependency must know what it can call, a bare module without a signature cannot be passed as a parameter. The default file-as-module form carries no parameters.

```/dev/null/db.sa#L1-6
mod db(io: storage.Storage)

use io::read
use io::query as io_query

pub fn connect(): Connection { read("config") }
pub fn query(q: String): List<Row> { io_query(q) }
```

The first line is the module header. It declares that `db` receives a value of type `storage.Storage` named `io`. Within the file, `io` is in scope and only its `Storage` interface is visible — the concrete implementation is not known to `db`.

The caller passes the concrete implementation at the `use` site:

```/dev/null/app.sa#L1-3
use db(storage.disk)::connect
use db(storage.disk)::query
```

The argument is resolved positionally against the module header's parameter list. Named arguments are also accepted, which is useful for clarity or when passing a subset of parameters:

```/dev/null/app.sa#L1-2
use db(io: storage.disk)::connect
```

The type checker verifies that the supplied module satisfies the declared sig at the `use` site, not at every call site. A different file can import the same module with a different implementation — no separate composition file is required.

For testing, a different implementation is passed at the same `use` site:

```/dev/null/app_test.sa#L1-3
use db(fake_storage)::connect
use db(fake_storage)::query
```

Where `fake_storage` is a module in the same directory that satisfies `storage.Storage`. Because arguments are resolved against the header's sig, the type checker catches mismatches before runtime.

When the same parameterised module is imported with two different implementations in the same file, the `as` alias distinguishes them:

```/dev/null/app.sa#L1-4
use db(storage.disk)::query
use db(storage.memory)::query as query_mem
```

The module header declaration forms are:

```/dev/null/forms.sa#L1-3
mod name                  -- file module, no params
mod name(dep: Sig)        -- module parameter with a sig contract
mod name(dep: Sig, dep2: other.Sig)  -- multiple parameters
```

Parameters may use either a named sig (`storage.Storage`) or a concrete module path as the contract (`storage.disk`), in which case the compiler derives the required surface from that module's public exports.

## LLM Context Bounding

The `in M` constraint on a generated function is the same mechanism as module parameterisation, applied at the function level. A function declared `in M` is a closed term whose free variables are drawn exclusively from M.

```/dev/null/strategy.sa#L1-4
sig StrategyApi {
    fn score(candidate: String): Int
    fn rank(scores: List<Int>): Int
}

fn generate_strategy(ctx: Context): (String -> Int) in StrategyApi
```

The formal reading, from CMTT, is `[StrategyApi ⊢ (String -> Int)]` — a function of type `String -> Int` in context `StrategyApi`. The LLM receives `StrategyApi`'s function types as its vocabulary. The runtime verifies that the generated body's free variables are a subset of `StrategyApi`'s exports before execution. This check is decidable and does not require running the generated code.

Context restriction allows a module that holds broad capabilities to hand a narrower context to generated code:

```/dev/null/agent.sa#L1-6
mod agent(io: storage.Storage, llm: llm.LlmApi)

fn safe_generate(ctx: Context): (String -> String) in io {
    llm.generate(ctx)!
}
```

`safe_generate` lives in a module with access to both `io` and `llm`. The generated function's context is restricted to `io`. The LLM call occurs at the meta level; what the generated code can do at the object level is narrower than the surrounding module's full context.

## Deferred Generation

SA distinguishes a value of type `A` from a value of type `deferred A` — code that produces an `A`, generated by the LLM and executed later. This is the `□A` modality of CMTT. Handwritten functions are never `deferred`. The type checker uses this distinction to reason about which boundary a computation crosses.

```/dev/null/example.sa#L1-3
fn generate_strategy(ctx: Context): deferred (String -> Int) in StrategyApi
```

The `deferred` annotation makes the LLM generation stage explicit in the type rather than implicit in a naming convention.

## Relationship to CMTT

Contextual Modal Type Theory introduces the notation `[Ψ ⊢ A]` for a term of type `A` in context `Ψ`. The `in M` clause is exactly `Ψ`. The module parameter mechanism is the surface syntax for constructing and passing contexts as typed values. CMTT's substitution rule — if you have `[M ⊢ A]` and a concrete implementation of `M`, substitution produces an `A` — is what makes swapping a mock for a real implementation type-safe. The `mod name: Sig = impl` form is that substitution made explicit.

SA's use of CMTT reverses its original purpose. CMTT was designed for meta-programming over a fixed object language where the context is known statically. In SA the context is fixed by the programmer and the term is generated at runtime by the model. The theory applies; the direction is novel.

```/dev/null/pipeline.txt#L1-6
Meta level:    SA program, module definitions, type checker
               ↓  LLM generates an AST bounded by in M
Object level:  [M ⊢ A]  — generated term checked in context M
               ↓  substitution: instantiate M, execute
Value level:   A  — runtime result
```

## Summary of Syntax

```/dev/null/syntax-summary.sa#L1-15
sig Name { ... }                            -- module interface type (optional)
mod name                                    -- file module, no params
mod name(dep: Sig)                          -- module header declaring a dependency
mod name(dep: Sig): Sig2 { ... }            -- inline module with a dependency and a sig assertion
use module::Name                            -- unqualified import
use module::Name as Alias                   -- named import
use module(impl)::Name                      -- import from parameterised module, positional arg
use module(dep: impl)::Name                 -- import from parameterised module, named arg
use module(impl1, impl2)::Name              -- multiple positional args
pub use module::Name                        -- re-export as part of this module's surface
pub use module::Name as Alias               -- re-export under a different name
pub use module(impl)::Name                  -- re-export from parameterised module
fn f(): T in M                              -- generated fn, context bounded to M
fn f(): deferred T in M                     -- explicitly marks LLM generation stage
```

## See Also

- Nanevski, Pfenning, Pientka. "Contextual Modal Type Theory." *ACM Transactions on Computational Logic*, 2008. https://dl.acm.org/doi/10.1145/1352582.1352591 — free PDF at https://www.cs.cmu.edu/~fp/papers/tocl07.pdf
- Rossberg, A. "1ML — Core and Modules United." *ICFP*, 2015. https://people.mpi-sws.org/~rossberg/1ml/
- Rossberg, A. and Dreyer, D. "Mixin' Up the ML Module System." *ACM Transactions on Programming Languages and Systems*, 2013. https://people.mpi-sws.org/~rossberg/mixml/
- Flatt, M. and Felleisen, M. "Units: Cool Modules for HOT Languages." *PLDI*, 1998. https://dl.acm.org/doi/10.1145/258949.258972 — Racket units; the explicit linking model
- Pierce, B. C. (2002). *Types and Programming Languages*. MIT Press. Chapter 24: existential types and abstract data types.
- Harper, R. (2016). *Practical Foundations of Programming Languages* (2nd ed.). Cambridge University Press. Chapter 44: modules and type abstraction. Free PDF at https://www.cs.cmu.edu/~rwh/pfpl/
- [docs/type-theory-llm-guardrails.md](type-theory-llm-guardrails.md) — CMTT, taint tracking, and the full guardrail model
- [src/structured-agent/ideas.md](../src/structured-agent/ideas.md) — original SA language design notes
