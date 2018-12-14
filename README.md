# Rustretto

### Building

#### Requirements

- LLVM 7.0

#### Building the runtime support library

```
$ RUSTFLAGS="-Cpanic=abort" cargo +nightly build --release -p runtime
```

#### Building the compiler

```
$ cargo build -p compiler
```

### Running

```
$ ./target/debug/compiler -r=target/release/libruntime.a -o=Main --main=Main Main.class
```
