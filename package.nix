{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "upower-notify";
  version = "dev";

  src = lib.cleanSource ./.;

  cargoHash = "sha256-Rm2ensgUMUc3nH5fMCAWbQVnuBlxkv8SxQ35scXpo30=";

  meta = {
    homepage = "https://github.com/Guanran928/upower-notify";
    license = lib.licenses.mit;
    mainProgram = "upower-notify";
  };
})
