{
  description = "Flatpak Helper (fh) - A wrapper for flatpak with keyword search (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      allSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs allSystems (system: f {
        pkgs = import nixpkgs { inherit system; };
      });
    in {
      packages = forAllSystems ({ pkgs }: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "flatpak-helper";
          version = "1.0.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postInstall = ''
            # Rename the binary to 'fh' as requested
            mv $out/bin/flatpak-helper $out/bin/fh
            
            # Wrap the binary to ensure flatpak is available
            wrapProgram $out/bin/fh \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.flatpak ]}
          '';
        };
      });

      apps = forAllSystems ({ pkgs }: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/fh";
        };
      });
    };
}
