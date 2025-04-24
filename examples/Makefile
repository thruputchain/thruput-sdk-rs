example_contracts : \
	erc20_low_level.wasm

other_contracts: \
	genesis_deployer.wasm \
	erc20.wasm \
	caller.wasm \
	simple_wallet.wasm \
	test_bytestring.wasm \
	playground.wasm

%.wasm : Makefile $(wildcard $</src/*.rs)
	cargo +nightly build \
		--features "guest" \
		--target wasm32-unknown-unknown \
		--release \
		--verbose \
		-p $(basename $@) \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort

ls_wasms:
	ls -l ../target/wasm32-unknown-unknown/release/*.wasm
.PHONY: ls_wasms
