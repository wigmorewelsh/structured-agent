
Very much an experiment. 

The idea behind structured agents is to treat the language model context window as a stack and use a structured programming language to drive the addition/removal of data from the window. Calling a function starts a new child context, content added in that scope will be removed when the function returns. There are implicit points in the language that the language model is called at, when a function completes without an explicit return value, when choosing functions to run using a select statement and when populating missing argument for function calls.

Because content is removed from the context when a function completes its simple to reimplement model multiple agent patterns without having to start to spin up multiple agents.

AI Agentistic loops are then recursive calls to a common function that selects between tools. Agent based workflows that currently use graphs can be re-written to be just function calls.

## License

The compiler and runtime are GPL v3 with the GCC Runtime Library Exception. In plain English: you can write closed-source programs with this language, but if you modify the compiler or runtime itself, those changes need to be open source.

Sample programs are MIT-0. Use them however you want.

See LICENSE for the legal text.

## Building

Requires Rust, some tests use Python. Standard cargo/uv workflow:

```bash
uv sync
cargo test
cargo build --release
```
