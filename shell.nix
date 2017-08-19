let
  nixpkgs-mozilla = (import <nixpkgs> {}).fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    rev = "ca0031baaac0538b9089625c8fa0b790b4270d36";
    sha256 = "0albnrwnx5ixgxvlrrcdyjsh5r25bqiw0xw7kdgi298inwyz3xz5";
  };

  rust-overlay = "${nixpkgs-mozilla}/rust-overlay.nix";
in

with import <nixpkgs> {
  overlays = [
    (import rust-overlay)
  ];
};

stdenv.mkDerivation rec {
  name = "irc-bot.rs";

  nativeBuildInputs = [
    (rustChannelOf {
      channel = "1.19.0";
    }).rust
    clang
    rustfmt
  ];

  buildInputs = [
  ];

  lib_path = lib.makeLibraryPath buildInputs;

  postFixup = ''
    for f in target/*/"$name"; do
      patchelf --set-rpath "$lib_path" "$f"
    done
  '';
}
