let
  inherit (import <nixpkgs> {})
    lib
    stdenv
    cargo
    clang
    rustc
    rustfmt
    ncurses
    openssl
  ;
in

stdenv.mkDerivation rec {
  name = "bot74d";

  nativeBuildInputs = [
    cargo
    clang
    rustc
    rustfmt
  ];

  buildInputs = [
    ncurses
    openssl
  ];

  lib_path = lib.makeLibraryPath buildInputs;

  postFixup = ''
    for f in target/*/"$name"; do
      patchelf --set-rpath "$lib_path" "$f"
    done
  '';
}
