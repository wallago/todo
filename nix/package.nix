{
  lib,
  naersk',
  src,
  release ? true,
}:
naersk'.buildPackage {
  pname = "todo";
  version = "0.1.0";
  inherit src release;
  cargoBuildOptions =
    opts:
    opts
    ++ [
      "-p"
      "todo-server"
    ];
  meta = {
    description = "Just a personal Todo app.";
    license = with lib.licenses; [
      asl20
      mit
    ];
  };
}
