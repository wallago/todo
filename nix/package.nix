{
  pkgs,
  lib,
  naersk',
  src,
  release ? true,
}:
naersk'.buildPackage {
  name = "todo";
  inherit src release;
  cargoBuildOptions =
    opts:
    opts
    ++ [
      "-p"
      "todo-server"
      # "-p"
      # "todo-mcp"
    ];
  meta = {
    description = "Just a personal Todo app.";
    license = with lib.licenses; [
      asl20
      mit
    ];
  };
}
