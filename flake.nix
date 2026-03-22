{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; };
    rust = pkgs.rust-bin.stable.latest.default;
  in {
    packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
      pname = "kagenti-native";
      version = "0.1.0";
      src = ./.;
      cargoLock.lockFile = ./Cargo.lock;
      nativeBuildInputs = [ pkgs.pkg-config ];
      buildInputs = [ pkgs.openssl ];
    };

    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [ rust pkgs.pkg-config pkgs.openssl pkgs.cargo-watch ];
    };
  };
}
