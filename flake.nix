{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      toolchainFile = pkgs.fetchurl {
        url = "https://raw.githubusercontent.com/jla2000/rust-gpu/refs/heads/main/rust-toolchain.toml";
        hash = "sha256-InRrQJIrxY9fxHgBN/lE+WEeTOOwDpFNrmoopeB9QSE=";
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        hardeningDisable = [ "fortify" ];
        nativeBuildInputs = [
          pkgs.spirv-tools
          (pkgs.rust-bin.fromRustupToolchainFile toolchainFile)
        ];
      };
    };
}
