
Very much an experiment. 

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

## Contributing

Contributions welcome. Same license applies to contributions.
