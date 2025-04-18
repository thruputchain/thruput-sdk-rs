build:
	cd ../ && make erc20.wasm
.PHONY: build

test:
	CC=gcc CXX=g++ \
	   cargo +nightly -vv test
.PHONY: test

expand:
	cargo +nightly expand \
		--features "guest,client" \
		--target wasm32-unknown-unknown
.PHONY: expand

tree:
	# Show the dependency graph
	cargo +nightly tree \
		--features "guest,client,mock" \
		--target wasm32-unknown-unknown \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort
.PHONY: tree
