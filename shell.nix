let
  nixpkgs-mozilla = (import <nixpkgs> {}).fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    # This revision is dated 2020-02-19.
    rev = "e912ed483e980dfb4666ae0ed17845c4220e5e7c";
    sha256 = "08fvzb8w80bkkabc1iyhzd15f4sm7ra10jn32kfch5klgl0gj3j3";
  };

  rust-overlay = "${nixpkgs-mozilla}/rust-overlay.nix";
in

with import <nixpkgs> {
  overlays = [
    (import rust-overlay)
  ];
};

let
  GitLab-CI-container-image-id = (lib.importJSON ./RUST_VERSION.yaml).image;
  Rust-version-str = lib.removePrefix "rust:" GitLab-CI-container-image-id;
in

stdenv.mkDerivation rec {
  name = "irc-bot.rs";

  nativeBuildInputs = [
    (rustChannelOf {
      channel = Rust-version-str;
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
