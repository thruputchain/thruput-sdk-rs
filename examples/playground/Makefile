build:
	cmake --build ~/gh/jan/build/hogd/rs_contracts/ -j

test:
	cd ../ && CC=gcc CXX=g++ \
	   cargo +nightly -v test -p playground -- --nocapture

test-client:
	cargo +nightly test \
		--no-default-features \
		--features "client,std" \

expand:
	cargo +nightly expand \
		--no-default-features \
		--features "guest,client,alloc" \
		--target wasm32-unknown-unknown

tree:
	# Show the dependency graph
	cargo +nightly tree \
		--no-default-features \
		--features "guest,alloc,client,mock" \
		--target wasm32-unknown-unknown \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort

.PHONY: build test test-client expand tree
