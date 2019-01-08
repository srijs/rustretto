# Rustretto

[![Build Status](https://travis-ci.com/srijs/rustretto.svg?branch=master)](https://travis-ci.com/srijs/rustretto)

## Getting started

### Building

#### Requirements

- Rust 1.31.0 (or higher)
- LLVM 7.0

#### Building the runtime support library

```
$ make runtime
```

#### Building the compiler

```
$ make compiler
```

### Running

```
$ ./target/debug/compiler -r=runtime/libruntime.a -o=Main --main=Main Main.class
```
