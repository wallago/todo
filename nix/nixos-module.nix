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
  # systemd copies the token here (LoadCredential), readable by the service only.
  credentials = "/run/credentials/todo-server.service";
  settings = lib.recursiveUpdate cfg.server.settings {
    server =
      lib.optionalAttrs (cfg.server.tokenFile != null) { token_file = "${credentials}/token"; }
      // lib.optionalAttrs (cfg.server.readTokenFile != null) {
        read_token_file = "${credentials}/read-token";
      };
  };
  configFile = settingsFormat.generate "todo-server.toml" settings;
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
          mcp.allowedHosts = [
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
      tokenFile = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "/run/agenix/todo-token";
        description = ''
          File holding the bearer token for /api and /mcp, e.g. an agenix or
          sops-nix secret. It can stay root-only. A string, not a Nix path,
          so the token stays out of the Nix store. null: no auth.
        '';
      };
      readTokenFile = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "/run/agenix/todo-read-token";
        description = ''
          File holding a read-only token for GET /api/agenda, also accepted
          as ?token= (phone widgets like KWGT can't set headers). Use a
          different value from tokenFile. Needs tokenFile.
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
        {
          assertion = cfg.server.readTokenFile == null || cfg.server.tokenFile != null;
          message = "services.todo.server.readTokenFile needs services.todo.server.tokenFile.";
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
        serviceConfig =
          hardening
          // {
            ExecStart = "${lib.getExe' cfg.package "todo-server"} --config ${configFile} ${vFlag cfg.server.verbose}";
            User = cfg.server.user;
            Group = cfg.server.group;
            # Notes it writes stay group-writable for the vault's other users.
            UMask = "0007";
            ReadWritePaths = [ cfg.server.settings.vault.root ];
          }
          // {
            LoadCredential =
              lib.optional (cfg.server.tokenFile != null) "token:${cfg.server.tokenFile}"
              ++ lib.optional (
                cfg.server.readTokenFile != null
              ) "read-token:${cfg.server.readTokenFile}";
          };
      };

      networking.firewall.interfaces = lib.genAttrs cfg.firewallInterfaces (_: {
        allowedTCPPorts = [ serverPort ];
      });
    })
  ];
}
