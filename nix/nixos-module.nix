self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.griffon-server;
  settingsFormat = pkgs.formats.toml { };
in
{
  options.packages."todo" = {
    enable = lib.mkEnableOption "todo";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Just a personal Todo app.";
    };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      example = { };
      description = ''
        Configuration written to the config.toml.
        Every key is optional; omitted keys keep the built-in defaults.
      '';
    };
    verbose = lib.mkOption {
      type = lib.types.ints.between 0 3;
      default = 1;
      example = 3;
      description = ''
        Verbosity level, passed to the app as repeated `-v` flags — `2` becomes `-vv`.

        - `0`: error / warn only, no `-v` flag
        - `1`: info
        - `2`: debug
        - `3`: traces
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ ];
  };
}
