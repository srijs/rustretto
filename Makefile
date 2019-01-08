.PHONY: runtime compiler

runtime:
	RUSTFLAGS="-Cpanic=abort" cargo build --release -p runtime

compiler:
	cargo build -p compiler

test: runtime
	cargo test
