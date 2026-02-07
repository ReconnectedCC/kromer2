{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    utils.url = "github:numtide/flake-utils/";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    utils,
    rust-overlay,
    ...
  }:
    utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      toolchain = pkgs.rust-bin.stable.latest;
    in {
      devShells.default = pkgs.mkShell {
        name = "kromer2 dev";

        packages = with pkgs; [
          toolchain.default
          sqlx-cli
          just
          dbeaver-bin
          atac
        ];

        RUST_BACKTRACE = "full";
      };
    });
}
