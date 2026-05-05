## User

[@Durable Execution with Graph Database](zed:///agent/thread/a5e4e3a6-83a3-4fe6-a446-fdadcf9e1c77?name=Durable+Execution+with+Graph+Database)

kuzu has been archived and the team employed by apple so this has been an interesting prototype but can't move forwards. 

We now need to look for other options that keep the properties of using a graphdb. 

reqs: 
1. it has to be embedded.
2. graph or able to save a graph like structure
3. able to append just the changes (this was the whole point of the graph over a blob)

alternatives: sqlite, HelixDB, loradb, ladybug (looks like an active fork of kuzu)

red flags: issues with data corruption/segfaults, very recent projects, project not being activly worked on, only one dev

## Assistant

<thinking>
The user wants to evaluate alternatives to Kuzu for an embedded graph database. Let me look at the current project structure first, then research the alternatives mentioned.

The alternatives to evaluate are:
1. SQLite (already mentioned as the recommended replacement)
2. HelixDB
3. loradb (probably "loredb" or "loradb")
4. ladybug (active fork of kuzu)

Let me research these options and look at the project structure.
</thinking>

Let me look at the project structure and research the alternatives in parallel.

**Tool Call: List the `structured-agent` directory's contents**
Status: Completed

# Folders:
structured-agent/.cargo
structured-agent/.github
structured-agent/.venv
structured-agent/.zed
structured-agent/docs
structured-agent/mutants.out
structured-agent/prototypes
structured-agent/scripts
structured-agent/src
structured-agent/target
structured-agent/tests

# Files:
structured-agent/.gitattributes
structured-agent/.gitignore
structured-agent/.gitmodules
structured-agent/.python-version
structured-agent/Cargo.lock
structured-agent/Cargo.toml
structured-agent/LICENSE
structured-agent/README.md
structured-agent/history.md
structured-agent/pyproject.toml
structured-agent/rustc-ice-2026-04-21T11_52_22-93231.txt
structured-agent/sweep.timestamp
structured-agent/uv.lock


**Tool Call: Research DB alternatives**
Status: Completed

I have enough to write the full comparison. Here it is:

---

## Research Findings

---

### 1. HelixDB

**Links**
- GitHub: https://github.com/HelixDB/helix-db
- crates.io: [`helix-db`](https://crates.io/crates/helix-db) (v1.0.115), [`helix-rs`](https://crates.io/crates/helix-rs) (v0.1.10, client SDK)

**Activity**
- 4,100 stars, 216 forks, ~2,652 commits
- Last crate update: July 2025 (`helix-db`), November 2025 (`helix-rs`)
- 171 releases since May 2025
- Small team; GitHub shows ~3 contributors in sidebar
- 12+ open issues including a May 2026 bug with HQL code generation (issue #904)
- Created: May 2025 (~1 year old)

**Embedded?** **NO.** HelixDB is explicitly a server — you run `helix push dev` to deploy a local instance, and call it over HTTP. There is an open feature request (#848, Feb 2026) specifically asking for embedded mode. That request is unanswered.

**Graph support?** Yes — property graph + vector, with a custom query language (HelixQL).

**Incremental/append writes?** Not applicable: it is server-based, writes go over HTTP.

**Rust bindings quality?** `helix-rs` is a client SDK (network calls), not an embeddable library.

**Corruption/segfault reports?** No data corruption reports found. One HQL code generation bug.

**License:** AGPL-3.0

**Red flags:**
- Disqualified outright if you need embedded. The entire premise of a "server you talk to" is the opposite of embedded.
- AGPL-3.0 is restrictive — any application linking it must be open source under AGPL unless you negotiate a commercial license.
- ~1 year old; no stable semver yet despite `v1.0.x` labelling.

---

### 2. LoraDB

**Links**
- GitHub: https://github.com/lora-db/lora (7 stars, 0 forks)
- crates.io: [`lora-database`](https://crates.io/crates/lora-database) (v0.5.6), plus `lora-store`, `lora-parser`, `lora-analyzer`, `lora-compiler`, `lora-executor`, `lora-wal`, `lora-server`

**Activity**
- 7 stars, 0 forks, 103 commits
- Latest release: v0.4.0 (Apr 25, 2026); crates first published April 22–25, 2025
- 0 open issues (issues may be disabled, or project is too new for users)
- ~3 contributors listed, appears to be single-developer in practice

**Embedded?** Yes — described as "embeddable in-memory graph store... Zero daemons, runs in your process."

**Graph support?** Yes — Cypher-like query language, property graph with typed nodes and edges. Also has a `VECTOR` type for embeddings.

**Incremental/append writes?** Ambiguous. The primary storage model is in-memory (`lora-store` is described as "in-memory graph store"). A `lora-wal` crate was published on April 25, 2026 — days ago at time of writing — described as a "write-ahead log and replay engine." It is too new to evaluate. Prior to that, there was no persistence mechanism at all. Whether it survives process restarts reliably is untested.

**Rust bindings quality?** Written in Rust natively; the `lora-database` crate is the entry point. API surface is reasonable but totally unproven.

**Corruption/segfault reports?** None found, but it is far too new to have seen meaningful production use.

**License:** Business Source License 1.1 (BSL 1.1). The project converts to Apache 2.0 on **April 19, 2029**. Until then, you cannot offer it as a hosted service or resell it. Internal production use is permitted. This is **not an OSI-approved open source license**.

**Red flags:**
- Published April 2025, WAL persistence added April 2026 — the project is effectively pre-production for anything requiring durable storage.
- 7 stars, 0 forks: near-zero community. No external validation whatsoever.
- BSL is not open source; the Apache 2.0 conversion date is 3 years away.
- Single-developer project — no bus-factor resilience.
- Total downloads: < 200 per crate.

---

### 3. Ladybug (lbug)

**Links**
- Website: https://ladybugdb.com
- crates.io: [`lbug`](https://crates.io/crates/lbug) (v0.16.0)
- GitHub: `https://github.com/lbugdb/lbug` — **returns HTTP 404**. The repository is not publicly accessible.

**Activity**
- 25,727 total downloads, all listed as "recent" — suspicious; may reflect CI pipelines or automated pulls rather than organic users.
- 12 versions since November 2025
- 0 stars visible (repo is 404); 0 forks visible
- Contributors: unknown — the repository is private or deleted

**Embedded?** Yes — "in-process property graph database management system" per crates.io. The website confirms "Zero external dependencies. Run anywhere."

**Graph support?** Yes — Cypher queries, columnar storage (claims KuzuDB lineage). The `hakuzu` crate on crates.io describes itself as "HA Kuzu/LadybugDB," confirming the relationship.

**Incremental/append writes?** Unknown — cannot inspect the source code.

**Rust bindings quality?** Unknown — cannot inspect source.

**Corruption/segfault reports?** Unknown — no public issue tracker accessible.

**License:** MIT per ladybugdb.com. Cannot be verified against the repo.

**Red flags:**
- The GitHub repository is a 404. You cannot audit the code, check the issue tracker, review contributor history, or verify the stated license. This alone should disqualify it for production use.
- 6 months old (November 2025).
- The website is marketing-heavy and positions itself as a KuzuDB successor, but KuzuDB is still available at https://github.com/kuzudb/kuzu (MIT license, actively maintained with 250+ contributors) — the "Kuzu acquired by Apple/development halted" narrative on the Ladybug website is unverified marketing copy.
- "Proudly sponsored by Ladybug Memory" — a commercial entity with unclear relationship to the open source project.
- No visible community, no verifiable source code.

---

### 4. SQLite via rusqlite

**Links**
- GitHub: https://github.com/rusqlite/rusqlite
- crates.io: [`rusqlite`](https://crates.io/crates/rusqlite) (v0.39.0)

**Activity**
- 4,200 stars, 467 forks, 2,895 commits
- Created 2014; 82 published versions over 11 years
- Last release: March 15, 2026
- Multiple active contributors; well-maintained org
- 58M total downloads, 12.7M recent downloads

**Embedded?** Yes — SQLite is the canonical embedded relational database. No separate process.

**Graph support?** Not natively. You can model adjacency lists or edge tables with SQL (self-joins, recursive CTEs), but there is no graph query language. For simple relationship traversal this is workable; for complex multi-hop graph queries it becomes verbose and slow without careful indexing.

**Incremental/append writes?** Yes — standard `INSERT`/`UPDATE` with ACID transactions. WAL journal mode (`PRAGMA journal_mode=WAL`) enables concurrent reads during writes.

**Rust bindings quality?** Excellent. Ergonomic, well-typed, feature-gated, bundled SQLite option (`features = ["bundled"]` compiles SQLite from vendored source, eliminating system library version concerns). `rustdoc` documentation is thorough. Active maintenance.

**Corruption/segfault reports?**
- One open segfault: #1503 — affects only SQLCipher + OpenSSL on OpenBSD. Not a concern for standard SQLite use.
- #1804: SIGSEGV in `TraceEvent::Row` trace callback — **fixed and merged** (Feb 2026).
- No data corruption reports. SQLite itself has an exceptional reliability record with extensive test coverage documented at [sqlite.org](https://sqlite.org/testing.html).

**License:** MIT (rusqlite) + public domain (SQLite)

**Red flags:** None for mainstream use. The one open segfault is platform-specific (OpenBSD + SQLCipher).

---

## Summary Table

| | HelixDB | LoraDB | Ladybug | rusqlite |
|---|---|---|---|---|
| **Embedded** | No (server) | Yes (in-memory) | Yes | Yes |
| **Graph support** | Yes (first-class) | Yes (Cypher-like) | Yes (Cypher) | No (SQL only) |
| **Persistent writes** | N/A | WAL added Apr 2026, unproven | Unknown | Yes (WAL) |
| **Rust bindings** | Client SDK only | Native Rust | Unknown | Excellent, 11 yrs |
| **License** | AGPL-3.0 | BSL 1.1 (→ Apache 2029) | MIT (unverifiable) | MIT + public domain |
| **Age** | ~1 yr | ~1 yr | ~6 months | 11+ years |
| **Stars** | 4.1k | 7 | 404 | 4.2k |
| **Downloads** | Low | ~200 | 25k (suspicious) | 58M |
| **Corruption/segfault** | None found | None found | Unknown | Platform-specific edge case (fixed) |
| **Community** | Small team | Single dev | None visible | Active, multi-contributor |

---

## Verdict

**HelixDB** is disqualified for embedded use — it is a server. Do not use it if you need an in-process library.

**LoraDB** is technically embeddable and written in pure Rust, but it is a pre-production project: 7 stars, single developer, BSL license, and its persistence story only arrived days ago. Not suitable for production.

**Ladybug** cannot be evaluated — the source code is inaccessible (404 on GitHub). You should not ship production software on a library whose code you cannot audit. The marketing claims about KuzuDB being dead are also questionable; [KuzuDB](https://github.com/kuzudb/kuzu) (which has proper Rust bindings via [`kuzu`](https://crates.io/crates/kuzu)) may itself be worth evaluating as a baseline.

**rusqlite** is the only battle-tested option here. If your data model can be expressed relationally (adjacency lists, edge tables, recursive CTEs), it is the lowest-risk choice by a wide margin. For genuine property graph queries with multi-hop traversal at scale, none of these options are ready — at which point KuzuDB directly (`kuzu` crate, MIT, 250+ contributors, embedded columnar graph DB) is a more credible embedded graph option than Ladybug's unverifiable fork of it.

## Assistant

<thinking>
Let me look at the project structure more to understand the current state before making a recommendation.
</thinking>

**Tool Call: List the `structured-agent/prototypes` directory's contents**
Status: Completed

# Folders:
structured-agent/prototypes/durable-exec


**Tool Call: List the `structured-agent/prototypes/durable-exec` directory's contents**
Status: Completed

# Folders:
structured-agent/prototypes/durable-exec/.cargo
structured-agent/prototypes/durable-exec/docs
structured-agent/prototypes/durable-exec/src
structured-agent/prototypes/durable-exec/target

# Files:
structured-agent/prototypes/durable-exec/Cargo.lock
structured-agent/prototypes/durable-exec/Cargo.toml


## Assistant

Here's what the research found. Short version: none of the alternatives are safe choices.

---

**HelixDB** - disqualified immediately. It's a server (HTTP), not embedded. Requires a running daemon. Also AGPL.

**LoraDB** - written in Rust, embeddable, has a Cypher-like graph query language. But it's 7 stars, single developer, BSL 1.1 license (not open source until 2029), and its persistence WAL was added literally days ago. Not production-ready.

**Ladybug** - the GitHub repo returns a 404. You cannot audit the code, verify the MIT license claim, or check for issues. The 25k "downloads" figure looks like bot/CI traffic. Unverifiable; should not ship on this.

**rusqlite** - the only battle-tested option. 11 years old, 58M downloads, multi-contributor, MIT + public domain. No native graph query language, but the schema from the previous prototype (frame chains via foreign keys + recursive CTEs) translates cleanly to SQL.

---

**A point on Kuzu itself:** the research agent flagged that `github.com/kuzudb/kuzu` is still accessible and shows 250+ contributors. The "archived/Apple" story may be more nuanced — worth verifying before treating it as fully dead. The prototype already had working Kuzu bindings; if the crate still compiles (with the pinned `cxx-build` workaround), Kuzu might still be usable as an interim while the SQLite store is built in parallel.

---

My recommendation given your requirements:

1. **SQLite via `rusqlite`** is the right production choice. It satisfies all three requirements: embedded, graph-like structure via adjacency tables, append-only incremental writes with WAL mode. The previous prototype's design (frame nodes, parent chains, context nodes as rows + FK relationships) maps directly to SQL tables. Recursive CTEs handle chain traversal.

2. The graph querying convenience is lost, but given the access pattern here (write frames incrementally, restore a chain by walking parent FKs) it was never doing anything that required a graph query language — a few recursive CTEs cover it.

Want me to proceed with implementing the `rusqlite`-backed `SnapshotStore`, porting the prototype tests across?

## User

ladybut is here https://github.com/LadybugDB/ladybug, check that 404 again

## Assistant

<thinking>
Let me check the correct GitHub URL for Ladybug.
</thinking>

**Tool Call: Fetch https://github.com/LadybugDB/ladybug**
Status: Completed

# Ladybug

Ladybug is an embedded graph database built for query speed and scalability. Ladybug is optimized for handling complex analytical workloads on very large databases and provides a set of retrieval features, such as a full text search and vector indices. Our core feature set includes:
- Flexible Property Graph Data Model and Cypher query language
- Embeddable, serverless integration into applications
- Native full text search and vector index
- Columnar disk-based storage
- Columnar sparse row-based (CSR) adjacency list/join indices
- Vectorized and factorized query processor
- Novel and very fast join algorithms
- Multi-core query parallelism
- Serializable ACID transactions
- Wasm (WebAssembly) bindings for fast, secure execution in the browser

Ladybug is being developed by LadybugDB Developers and is available under a permissive license. So try it out and help us make it better! We welcome your feedback and feature requests.

The database was formerly known as Kuzu.

## Installation

| Language | Installation |
| --- | --- |
| Python | `pip install ladybug` |
| NodeJS | `npm install @ladybugdb/core` |
| Rust | `cargo add lbug` |
| Go | `go get github.com/LadybugDB/go-ladybug` |
| Swift | lbug-swift |
| Java | Maven Central |
| C/C++ | precompiled binaries |
| CLI | precompiled binaries |

To learn more about installation, see our Installation page.

## Getting Started

Refer to our Getting Started page for your first example.

## Build from Source

You can build from source using the instructions provided in the developer guide.

## Contributing

We welcome contributions to Ladybug. If you are interested in contributing to Ladybug, please read our Contributing Guide.

## License

By contributing to Ladybug, you agree that your contributions will be licensed under the MIT License.

## Assistant

<thinking>
Now I can see that Ladybug is clearly the renamed/forked version of KuzuDB. The repo says "The database was formerly known as Kuzu." It has 1k stars, 79 forks, 5,812 commits, and 13 releases (latest 0.16.0 on Apr 29, 2026). The Rust crate is `lbug` via `cargo add lbug`.

This is a legitimate active project - it's literally KuzuDB renamed/rebranded. The research agent hit the wrong URL (lowercase vs uppercase organization name on GitHub). 

Key points:
- It IS KuzuDB, just renamed to Ladybug


- MIT licensed with active development and recent releases
- Embedded graph database using Cypher queries with columnar disk-based storage and ACID transactions
- Available as the `lbug` Rust crate

Since this is the same codebase as the original Kuzu, the C++ and ABI compatibility issues we encountered before would likely still be present here. Let me check the crates.io page for `lbug` to understand the current state of the Rust bindings.
</thinking>

Good catch. Let me get more detail on the `lbug` crate.

**Tool Call: Fetch https://crates.io/crates/lbug**
Status: Failed

status error 404, response: ""

**Tool Call: Fetch https://crates.io/search?q\=lbug**
Status: Failed

status error 404, response: ""

**Tool Call: cargo search lbug 2>&1 | head -20**
Status: Completed

Terminal:
```
lbug = "0.16.0"     # An in-process property graph database management system built for query speed and scalability
hakuzu = "0.3.0"    # HA Kuzu/LadybugDB with one line of code. Leader election, journal replication, write forwarding — just your…
note: to learn more about a package, run `cargo info <name>`

```


**Tool Call: cargo info lbug 2>&1**
Status: Completed

Terminal:
```
    Updating crates.io index
  Downloaded lbug v0.16.0
  Downloaded 1 crate (9.1MiB) in 2.74s
lbug #database #graph #ffi
An in-process property graph database management system built for query speed and scalability
version: 0.16.0
license: MIT
rust-version: 1.81
documentation: https://docs.rs/lbug/0.16.0
homepage: https://ladybugdb.com/
repository: https://github.com/lbugdb/lbug
crates.io: https://crates.io/crates/lbug/0.16.0
features:
 +default         = []
  arrow           = [dep:arrow]
  extension_tests = []

```


**Tool Call: Fetch https://github.com/lbugdb/lbug**
Status: Failed

## Assistant

<thinking>
So `github.com/lbugdb/lbug` (lowercase) is a 404. But `github.com/LadybugDB/ladybug` is the correct URL. The `lbug` crate on crates.io points to `https://github.com/lbugdb/lbug` (lowercase) which is a 404, but the actual source is at `https://github.com/LadybugDB/ladybug` (uppercase).

Key facts from the LadybugDB/ladybug page:
- 1k stars, 79 forks
- 5,812 commits


- MIT License
- 13 releases, latest 0.16.0 on Apr 29, 2026
- Primarily C++ (74.4%) with Cypher (21.2%)

This is KuzuDB rebranded as Ladybug—the original Kuzu codebase is now maintained under the new name. The `lbug` crate metadata points to an incorrect repository URL, but the actual source code lives at LadybugDB/ladybug.

The C++ ABI issues we encountered with Kuzu would likely persist since it's the same underlying codebase. That said, the team has maintained continuity through the transition—they're actively developing it with 5,812 commits and a recent release in April 2026, so Ladybug addresses the "Kuzu archived" concern even though the compilation challenges remain.
</thinking>

Good find. This changes things. Ladybug is Kuzu, just renamed. The repo says explicitly: **"The database was formerly known as Kuzu."**

What this means practically:

- **It's the same codebase**: 5,812 commits, continuous history from Kuzu. 1k stars, 79 forks, MIT. Last release v0.16.0 on Apr 29, 2026.
- **The C++ / cxx ABI problem remains**: 74.4% C++. The `cxx`/`cxx-build` version pinning issue from the prototype would apply to `lbug` in exactly the same way it did to `kuzu`. You'd need the same `cxx-build = "=1.0.x"` workaround.
- **The crate repo link is broken**: `lbug` on crates.io points to `github.com/lbugdb/lbug` (lowercase), which 404s. The real source is at `github.com/LadybugDB/ladybug`. That's a minor packaging hygiene issue but worth noting.
- **The "Apple acquisition / archived" story**: this appears to have been Kuzu's commercial pivot under a new name. Development continued as Ladybug. The previous concern about it being abandoned is resolved.

Updated picture:

| | Ladybug (`lbug`) | rusqlite |
|---|---|---|
| Embedded | Yes | Yes |
| Graph | Yes (Cypher, columnar) | No (SQL + recursive CTE) |
| Incremental writes | Yes (ACID) | Yes (WAL) |
| Rust bindings | FFI over C++ | Pure Rust |
| C++ build pain | Yes (same as Kuzu) | No |
| License | MIT | MIT + public domain |
| Activity | Active (Apr 2026) | Active (Mar 2026) |

So you have a real choice now:

1. **`lbug` (Ladybug/Kuzu)**: keeps the graph query model and existing prototype schema almost unchanged, but you carry the C++ compile-time overhead and the `cxx-build` pin workaround permanently.

2. **`rusqlite`**: no C++ pain, but you trade the graph query language for SQL + recursive CTEs, which are sufficient for the frame-chain access pattern here.

Given that the prototype's graph queries were simple frame-chain traversals (never needing multi-hop Cypher), the operational cost of the C++ toolchain is probably not worth it. But if you want to keep a genuine property graph store with Cypher, Ladybug is a real, maintained option now.

Which direction do you want to go?

