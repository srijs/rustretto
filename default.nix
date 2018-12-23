let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
in
  with import <nixpkgs> { overlays = [ moz_overlay ]; };

stdenv.mkDerivation rec {
  name = "env";

  env = buildEnv { name = name; paths = buildInputs; };

  platformBuildInputs =
    if hostPlatform.isDarwin
    then [ darwin.apple_sdk.frameworks.Security ]
    else [ ];

  buildInputs = platformBuildInputs ++ [
    latest.rustChannels.stable.rust

    llvm_7
    openjdk8
  ];

  RUST_BACKTRACE = 1;
}
