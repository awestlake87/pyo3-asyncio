
clippy:
	@touch src/lib.rs  # Touching file to ensure that cargo clippy will re-check the project
	cargo clippy --tests -- -Dwarnings
	cargo clippy  --tests -- -Dwarnings
	# for example in examples/*; do cargo clippy --manifest-path $$example/Cargo.toml -- -Dwarnings || exit 1; done

fmt:
	cargo fmt --all -- --check

lint: fmt clippy
	@true

test: lint
	cargo test --all-features

publish: test
	cargo publish
