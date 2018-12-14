# Rustretto

### Building

#### Requirements

- LLVM

#### Building the runtime support library

```
$ RUSTFLAGS="-Cpanic=abort" cargo +nightly build --release -p runtime
```

#### Building the compiler

```
$ cargo run -p compiler -- -r=target/release/libruntime.a -o=Main --main=Main Main.class
```
