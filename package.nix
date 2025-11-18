{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "upower-notify";
  version = "dev";

  src = lib.cleanSource ./.;

  cargoHash = "sha256-Ls9fWVDNKgKTdG+Hozb8X6krlIrvGKdSKRHp+aDZUqU=";

  meta = {
    homepage = "https://github.com/Guanran928/upower-notify";
    license = lib.licenses.mit;
    mainProgram = "upower-notify";
  };
})
