# CARGO_TARGET_DIR is set in CMakeLists.txt,
# so to build these contracts, use:
#
#   cmake --build ${BUILD}/hogd/rs_contracts/ -j

rs_contracts : \
	genesis_deployer.wasm \
	all_rs_contracts
.PHONY: rs_contracts

all_rs_contracts : \
	genesis_deployer.wasm \
	erc20.wasm \
	erc20_low_level.wasm \
	caller.wasm \
	rcq_test.wasm \
	simple_wallet.wasm \
	test_bytestring.wasm \
	playground.wasm
.PHONY: all_rs_contracts

%.wasm : Makefile $(wildcard $</src/*.rs)
	cargo +nightly rustc \
		--no-default-features \
		--features "guest,alloc" \
		--target wasm32-unknown-unknown \
		--target-dir $(CARGO_TARGET_DIR) \
		--release \
		--verbose \
		-p $(basename $@) \
		-Zbuild-std=alloc,core \
		-Zbuild-std-features=panic_immediate_abort \
		-- \
		-C link-args=-zstack-size=64000

test:
	(cd erc20_low_level && make test) && \
		(cd erc20 && make test) && \
		(cd simple_wallet && make test) && \
		(cd caller && make test) && \
		(cd playground && make test) && \
.PHONY: test

clean:
	cargo clean
.PHONY: clean
