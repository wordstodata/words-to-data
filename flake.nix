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

        # Python bindings
        pkgs.python313
        pkgs.maturin

        # Extra python packages
        pkgs.python313Packages.tqdm

        # Needed to build packages
        pkgs.git
        pkgs.git-lfs

        # nicer commits
        pkgs.commitizen

        # Pre-commit hooks
        pkgs.pre-commit

        # Annotation tool (Tauri desktop app)
        pkgs.nodejs
        pkgs.webkitgtk_4_1   # WebKit2GTK webview engine (libsoup3 variant)
        pkgs.gtk3             # GTK3 window management
        pkgs.libsoup_3        # HTTP for WebKit
        pkgs.glib-networking  # TLS support for WebKit
        pkgs.openssl          # TLS
        pkgs.librsvg          # SVG rendering
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

      # Tauri bug: https://github.com/tauri-apps/tauri/issues/13493
      WEBKIT_DISABLE_DMABUF_RENDERER=1;

      # Without this, will throw a "No GSettings schemas are installed on the system" when opening a dialog box
      GSETTINGS_SCHEMA_DIR="${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}/glib-2.0/schemas";
    };

  };
}
