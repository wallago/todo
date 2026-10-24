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

  buildInputs = with pkgs; [
    pkg-config
    udev
  ];
  meta = {
    description = "Just a personal Todo app.";
    license = with lib.licenses; [
      asl20
      mit
    ];
  };
}
