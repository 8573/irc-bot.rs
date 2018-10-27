
build:
	@nix-shell --run 'cargo build --color=always' 2>&1 | $(PAGER)

release-build:
	@nix-shell --run 'cargo build --release --color=always' 2>&1 | $(PAGER)

check:
	@nix-shell --run 'cargo check --color=always' 2>&1 | $(PAGER)

test:
	@nix-shell --run 'cargo test --color=always' 2>&1 | $(PAGER)

run:
	@nix-shell --run 'cargo run'

release-run:
	@nix-shell --run 'cargo run --release'

docs:
	@nix-shell --run 'cargo doc --no-deps --all-features'

view-docs:
	@nix-shell --run '\
	   BROWSER=chromium-browser \
	   cargo doc --no-deps --all-features --open \
	 '

clean:
	@nix-shell --run 'cargo clean'

publish:
	@nix-shell --run 'cargo test && cargo publish'

fmt:
	@nix-shell --run 'cargo-fmt'

shell:
	@nix-shell
