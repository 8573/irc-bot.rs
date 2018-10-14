let
  nixpkgs-mozilla = (import <nixpkgs> {}).fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    # This revision is dated 2018-10-08.
    rev = "c72ff151a3e25f14182569679ed4cd22ef352328";
    sha256 = "0akyhdv5p0qiiyp6940k9bvismjqm9f8xhs0gpznjl6509dwgfxl";
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
      channel = "1.26.2";
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
