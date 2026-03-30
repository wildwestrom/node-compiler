{
  description = "node-compiler";

  inputs = {
    # We do not add a nixpkgs url but just use whatever is on the current NixOS system.
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        libPath =
          with pkgs;
          lib.makeLibraryPath [
            libGL
            libxkbcommon
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
          ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ rust-toolchain ];
          buildInputs = with pkgs; [
            libGL
            libxkbcommon
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
            libxcb

            rusty-man
          ];
          LD_LIBRARY_PATH = libPath;
          RUST_LOG = "node_compiler=trace,warn";
        };
      }
    );
}
