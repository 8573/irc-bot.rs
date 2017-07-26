
build:
	@nix-shell --run 'cargo build'

run:
	@nix-shell --run 'cargo run'

clean:
	@nix-shell --run 'cargo clean'

fmt:
	@nix-shell --run 'cargo-fmt'

shell:
	@nix-shell
