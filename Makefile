
build:
	@nix-shell --run 'cargo build'

release-build:
	@nix-shell --run 'cargo build --release'

check:
	@nix-shell --run 'cargo check'

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

fmt:
	@nix-shell --run 'cargo-fmt'

shell:
	@nix-shell
