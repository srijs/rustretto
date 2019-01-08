.PHONY: runtime compiler clean

runtime:
	make -C runtime

compiler:
	cargo build -p compiler

test: runtime
	cargo test

clean:
	make -C runtime clean
