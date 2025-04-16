check:
	cargo +nightly check --no-default-features --features=std
.PHONY: check

test:
	cargo +nightly test --jobs=1 --no-default-features --features=mock
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
