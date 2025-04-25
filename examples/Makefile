example_contracts : \
	genesis_deployer.wasm \
	erc20_low_level.wasm \
	erc20.wasm \
	caller.wasm \
	test_bytestring.wasm \
	playground.wasm

# missing a syscall:
#    simple_wallet.wasm

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
