build:
	cd ../ && make erc20.wasm
.PHONY: build

test:
	CC=gcc CXX=g++ \
	   cargo +nightly -vv test
.PHONY: test

test-client:
	CC=gcc CXX=g++ \
		cargo +nightly test \
			--no-default-features \
			--features "client,std" \
.PHONY: test-client

expand:
	cargo +nightly expand \
		--no-default-features \
		--features "guest,client,alloc" \
		--target wasm32-unknown-unknown
.PHONY: expand

tree:
	# Show the dependency graph
	cargo +nightly tree \
		--no-default-features \
		--features "guest,alloc,client,mock" \
		--target wasm32-unknown-unknown \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort
.PHONY: tree
