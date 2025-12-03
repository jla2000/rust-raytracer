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
        config.allowUnfree = true;
        config.microsoftVisualStudioLicenseAccepted = true;
      };
      toolchainFile = pkgs.fetchurl {
        url = "https://raw.githubusercontent.com/jla2000/rust-gpu/refs/heads/main/rust-toolchain.toml";
        hash = "sha256-InRrQJIrxY9fxHgBN/lE+WEeTOOwDpFNrmoopeB9QSE=";
      };
    in
    {
      devShells.${system} = {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            spirv-tools
            (rust-bin.fromRustupToolchainFile toolchainFile)
          ];
          LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
            vulkan-loader
            libxkbcommon
            xorg.libXi
            xorg.libX11
            xorg.libXcursor
          ];
        };
        windows = pkgs.pkgsCross.mingwW64.mkShell {
          nativeBuildInputs = with pkgs; [
            spirv-tools
            ((rust-bin.fromRustupToolchainFile toolchainFile).override {
              targets = [ "x86_64-pc-windows-gnu" ];
            })
          ];
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${pkgs.pkgsCross.mingwW64.windows.pthreads}/lib";
        };
      };
    };
}
