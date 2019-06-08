let
  nixpkgs-mozilla = (import <nixpkgs> {}).fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    # This revision is dated 2019-05-09.
    rev = "33bda5d711a82a2b511262ef3be367a86ef880df";
    sha256 = "0lbb22paqsn3g0ajxzw4vj7lbn9ny2vdkp5sqm3a7wrc56a8r35b";
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
      channel = "1.34.2";
    }).rust
    clang
    git
    pkgconfig
  ];

  buildInputs = [
    openssl
  ];

  lib_path = lib.makeLibraryPath buildInputs;

  postFixup = ''
    for f in target/*/"$name"; do
      patchelf --set-rpath "$lib_path" "$f"
    done
  '';
}
