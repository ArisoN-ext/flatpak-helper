{
  description = "Flatpak Helper (fh) - A wrapper for flatpak with keyword search (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: {
    packages =
      nixpkgs.lib.genAttrs
        [
          "x86_64-linux"
          "aarch64-linux"
        ]
        (system: {
          default = nixpkgs.legacyPackages.${system}.callPackage ./default.nix { };
        });
  };
}
