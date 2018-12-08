with import <nixpkgs> {};

let
  frameworks = darwin.apple_sdk.frameworks;
in stdenv.mkDerivation rec {
  name = "env";

  env = buildEnv { name = name; paths = buildInputs; };

  buildInputs = [
    frameworks.Security

    libxml2
    llvm_7
  ];
}
