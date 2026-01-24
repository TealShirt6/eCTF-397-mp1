{
  description = "ECE 397 eCTF MP1 Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      flake-utils,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem = f: nixpkgs.lib.genAttrs supportedSystems (system: f system);
    in
    {
      devShells = forEachSupportedSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          target = "thumbv6m-none-eabi";
          rustToolchain = fenix.packages.${system}.combine [
            fenix.packages.${system}.complete.toolchain
            fenix.packages.${system}.targets.${target}.latest.rust-std
            fenix.packages.${system}.complete.rust-src
            fenix.packages.${system}.complete.rust-analyzer
          ];
        in
        {
          default = pkgs.mkShell {
            buildInputs = [
              rustToolchain
              pkgs.probe-rs-tools
              pkgs.git
              pkgs.bashInteractive
              pkgs.pkg-config
              pkgs.udev
              pkgs.picocom
            ];

            RUST_SRC_PATH = "${fenix.packages.${system}.complete.rust-src}/lib/rustlib/src/rust/library";
          };
        }
      );
    };
}
