with import <nixpkgs> {};

stdenv.mkDerivation rec {
  name = "env";

  env = buildEnv { name = name; paths = buildInputs; };

  platformBuildInputs =
    if hostPlatform.isDarwin
    then [ darwin.apple_sdk.frameworks.Security ]
    else [ ];

  buildInputs = platformBuildInputs ++ [ libxml2 llvm_7 ];
}
