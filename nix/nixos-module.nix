self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.todo;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "todo-server.toml" cfg.server.settings;
  vFlag = n: lib.optionalString (n > 0) "-${lib.concatStrings (lib.replicate n "v")}";
  serverPort = cfg.server.settings.server.port or 3000;
  verbose = lib.mkOption {
    type = lib.types.ints.between 0 3;
    default = 1;
    description = "0 = warn, 1 = info, 2 = debug, 3 = trace.";
  };
  hardening = {
    NoNewPrivileges = true;
    PrivateTmp = true;
    ProtectSystem = "strict";
    ProtectKernelTunables = true;
    ProtectControlGroups = true;
    RestrictSUIDSGID = true;
    Restart = "on-failure";
  };
in
{

  options.services.todo = {
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Package with the todo-server binary.";
    };

    server = {
      enable = lib.mkEnableOption "the todo server";
      inherit verbose;
      user = lib.mkOption {
        type = lib.types.str;
        default = "todo";
        description = "User the server runs as. Needs write access to the vault.";
      };
      group = lib.mkOption {
        type = lib.types.str;
        default = "todo";
      };
      settings = lib.mkOption {
        type = settingsFormat.type;
        example = {
          server.port = 3000;
          vault = {
            root = "/srv/vault";
            inbox = "todo.md";
          };
          # Host names MCP clients use for /mcp.
          mcp.allowed_hosts = [
            "localhost"
            "127.0.0.1"
            "::1"
            "todo.wallago.xyz"
          ];
        };
        description = ''
          Written to config.toml. `vault.root` must be absolute:
          a relative path would resolve inside the Nix store.
        '';
      };
    };

    firewallInterfaces = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "wg0" ];
      description = "Open the server port on these interfaces only.";
    };
  };

  config = lib.mkMerge [
    (lib.mkIf cfg.server.enable {
      assertions = [
        {
          assertion = lib.hasPrefix "/" (cfg.server.settings.vault.root or "");
          message = "services.todo.server.settings.vault.root must be an absolute path.";
        }
      ];

      users.users = lib.mkIf (cfg.server.user == "todo") {
        todo = {
          isSystemUser = true;
          group = cfg.server.group;
        };
      };
      users.groups = lib.mkIf (cfg.server.group == "todo") { todo = { }; };

      systemd.services.todo-server = {
        description = "Todo server";
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = hardening // {
          ExecStart = "${lib.getExe' cfg.package "todo-server"} --config ${configFile} ${vFlag cfg.server.verbose}";
          User = cfg.server.user;
          Group = cfg.server.group;
          # Notes it writes stay group-writable for the vault's other users.
          UMask = "0007";
          ReadWritePaths = [ cfg.server.settings.vault.root ];
        };
      };

      networking.firewall.interfaces = lib.genAttrs cfg.firewallInterfaces (_: {
        allowedTCPPorts = [ serverPort ];
      });
    })
  ];
}
