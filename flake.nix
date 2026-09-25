{
  description = "MolGFX development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          python = pkgs.python313;

          linuxGpuPackages =
            pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.vulkan-loader
              pkgs.vulkan-tools
            ];
        in
        {
          default = pkgs.mkShell {
            packages =
              [
                rust

                # Python / PyO3
                python
                pkgs.uv
                pkgs.maturin

                # Browser runtime
                pkgs.wasm-pack

                # Repository utilities
                pkgs.git
                pkgs.pkg-config
                pkgs.jq
              ]
              ++ linuxGpuPackages;

            UV_PYTHON = "${python}/bin/python3";

            RUST_BACKTRACE = "1";
            CARGO_TERM_COLOR = "always";

            shellHook = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              export LD_LIBRARY_PATH="${
                pkgs.lib.makeLibraryPath [ pkgs.vulkan-loader ]
              }:''${LD_LIBRARY_PATH:-}"
            '';
          };
        }
      );
    };
}
