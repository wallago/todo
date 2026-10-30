self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.todo;
  settingsFormat = pkgs.formats.toml { };
in
{
  options.programs."todo" = {
    enable = lib.mkEnableOption "todo TUI";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.tui;
      description = "Package with the todo-tui binary.";
    };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = "Written to config.toml. token_file must be absolute.";
      example = {
        server = {
          url = "http://127.0.0.1:3000";
          token_file = "/path/token/file";
        };
        keys = {
          up = "k";
          down = "j";
          left = "h";
          right = "l";
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
    xdg.configFile."todo-tui/config.toml" = lib.mkIf (cfg.settings != { }) {
      source = settingsFormat.generate "todo-tui.toml" cfg.settings;
    };
  };
}
