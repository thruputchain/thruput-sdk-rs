check:
	cargo +nightly check --features=std
.PHONY: check

check_guest:
	cargo +nightly check --features=std,guest
.PHONY: check_guest

test:
	cargo +nightly test --jobs=1 --features=mock
.PHONY: test

lint:
	cargo +nightly clippy --no-deps --features=std
.PHONY: lint

lint_mock:
	cargo +nightly clippy --no-deps --features=mock
.PHONY: lint_mock

lint_macros:
	cargo +nightly clippy --no-deps -p macros
.PHONY: lint_macros

doc:
	cargo +nightly doc --features=std
.PHONY: doc

doc_open:
	cargo +nightly doc --features=std --open
.PHONY: doc_open

fmt:
	cargo +nightly fmt
.PHONY: fmt

example_contracts:
	make -C eg example_contracts
.PHONY: example_contracts

test_contracts:
	./scripts/test_example_contracts.sh
.PHONY: test_contracts

tree:
	# Show the dependency graph
	cargo +nightly tree \
		--edges normal,build \
		--target wasm32-unknown-unknown
