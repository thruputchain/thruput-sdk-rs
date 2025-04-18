example_contracts : \
	genesis_deployer.wasm \
	erc20.wasm \
	erc20_low_level.wasm \
	caller.wasm \
	rcq_test.wasm \
	simple_wallet.wasm \
	test_bytestring.wasm \
	playground.wasm

%.wasm : Makefile $(wildcard $</src/*.rs) $(wildcard $</src/storage/*.rs)
	cargo +nightly build \
		--features "guest" \
		--target wasm32-unknown-unknown \
		--release \
		--verbose \
		-p $(basename $@) \
		-Z build-std=core,alloc \
		-Z build-std-features=panic_immediate_abort
	# Flags for rustc are attached to the release profile
	# using `rustflags` in `Cargo.toml`

ls_wasms:
	ls -l ../target/wasm32-unknown-unknown/release/*.wasm
.PHONY: ls_wasms
