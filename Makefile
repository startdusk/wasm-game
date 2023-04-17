.PHONY: codeline
codeline:
	@tokei .

.PHONY: test 
test: fmt
	@cargo nextest run

.PHONY: fmt
fmt:
	@cargo fmt -- --check && cargo clippy --all-targets --all-features --tests --benches -- -D warnings


.PHONY: run
run:
	@npm run start
