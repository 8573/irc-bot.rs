let
  inherit (import <nixpkgs> {})
    lib
    stdenv
    cargo
    clang
    rustc
    rustfmt
    openssl
    pkgconfig
  ;
in

stdenv.mkDerivation rec {
  name = "irc-bot.rs";

  nativeBuildInputs = [
    cargo
    clang
    rustc
    rustfmt
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
