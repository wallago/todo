{
  lib,
  naersk',
  src,
  crate,
  release ? true,
}:
naersk'.buildPackage {
  pname = crate;
  version = "0.1.0";
  inherit src release;
  cargoBuildOptions =
    opts:
    opts
    ++ [
      "-p"
      crate
    ];
  mainProgram = crate;
  meta = {
    description = "Just a personal Todo app.";
    license = with lib.licenses; [
      asl20
      mit
    ];
  };
}
