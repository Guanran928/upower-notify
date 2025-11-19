{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "upower-notify";
  version = "dev";

  src = lib.cleanSource ./.;

  cargoHash = "sha256-b2xZfo3h/jp3YXWs8RZFrZrGImenX8Td9Gk/guNsZpo=";

  meta = {
    homepage = "https://github.com/Guanran928/upower-notify";
    license = lib.licenses.mit;
    mainProgram = "upower-notify";
  };
})
