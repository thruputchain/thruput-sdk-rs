build:
	cd ../ && make erc20_low_level.wasm

test:
	CC=gcc CXX=g++ \
	   cargo +nightly -vv test

test-client:
	cargo +nightly test \
		--no-default-features \
		--features "client,std" \

expand:
	cargo +nightly expand \
		--no-default-features \
		--features "guest,alloc,client,mock" \

tree:
	# Show the dependency graph
	cargo +nightly tree \
		--no-default-features \
		--features "guest,alloc" \
		--target wasm32-unknown-unknown \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort

.PHONY: build test test-client expand tree
