{
  lib,
  rustPlatform,
  makeWrapper,
  flatpak,
}:

rustPlatform.buildRustPackage {
  pname = "flatpak-helper";
  version = "1.0.1";

  __structuredAttrs = true;

  src = lib.cleanSource ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    mv $out/bin/flatpak-helper $out/bin/fh

    wrapProgram $out/bin/fh \
      --prefix PATH : ${lib.makeBinPath [ flatpak ]}
  '';

  meta = {
    description = "A fast, intelligent, and seamless CLI wrapper for Flatpak written in Rust";
    homepage = "https://github.com/ArisoN-ext/flatpak-helper";
    license = lib.licenses.gpl3Only;
    mainProgram = "fh";
  };
}
