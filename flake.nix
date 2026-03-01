{
  description = "W2D Flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }:
  let
    pkgs = import nixpkgs { system = "x86_64-linux";};

    stdInputs = [
        # Rust
        pkgs.cargo
        pkgs.rustc
        pkgs.pkg-config
        pkgs.glib

        # Needed to build packages
        pkgs.git

    ];
    devInputs = [
        pkgs.rustfmt
        pkgs.clippy
        pkgs.rust-analyzer
    ];
  in
  {
    devShells."x86_64-linux".default = pkgs.mkShell {
       buildInputs = stdInputs ++ devInputs;

      # Rust stdlib for language servers
      RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    };

  };
}
