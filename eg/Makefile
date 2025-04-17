example_contracts : \
	genesis_deployer.wasm \
	erc20.wasm \
	erc20_low_level.wasm \
	caller.wasm \
	rcq_test.wasm \
	simple_wallet.wasm \
	test_bytestring.wasm \
	playground.wasm

%.wasm : Makefile $(wildcard $</src/*.rs)
	cargo +nightly rustc \
		--no-default-features \
		--features "guest" \
		--target wasm32-unknown-unknown \
		--release \
		--verbose \
		-p $(basename $@) \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort \
		-- \
		-C link-args=-zstack-size=64000

ls_wasms:
	ls -l ../target/wasm32-unknown-unknown/release/*.wasm
.PHONY: ls_wasms
