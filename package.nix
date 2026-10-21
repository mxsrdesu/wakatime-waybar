{ lib, rustPlatform, makeWrapper, procps, xdg-utils, wakatimeDesktop ? null }:
rustPlatform.buildRustPackage {
  pname = "waybar-wakatime";
  version = "1.0.1";
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./src ./tests ./assets ./examples ];
  };
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ makeWrapper ];
  strictDeps = true;
  doCheck = true;
  postInstall = ''
    wrapProgram "$out/bin/waybar-wakatime" \
      --prefix PATH : ${lib.makeBinPath ([ procps xdg-utils ] ++ lib.optional (wakatimeDesktop != null) wakatimeDesktop)} ${lib.optionalString (wakatimeDesktop != null) "--set-default WAKATIME_DESKTOP_EXECUTABLE ${wakatimeDesktop}/bin/wakatime-desktop"}
    mkdir -p "$out/share/waybar-wakatime"
    cp assets/* "$out/share/waybar-wakatime/"
    cp examples/waybar.jsonc "$out/share/waybar-wakatime/"
  '';
  meta = {
    description = "wakatime daily total for waybar";
    license = lib.licenses.bsd3;
    platforms = lib.platforms.linux;
    mainProgram = "waybar-wakatime";
  };
}
