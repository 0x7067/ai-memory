# A small discoverability layer for common non-secret config.toml keys.
#
# The parent submodule has `freeformType = pkgs.formats.toml.type`, so every
# current and future config key remains usable without copying Config's full
# Rust schema into Nix. Secrets belong in an environment file, never here.
{ lib, ... }:

let
  inherit (lib) types;
in
{
  allowed_hosts = lib.mkOption {
    type = types.nullOr (types.listOf types.str);
    default = null;
    description = "Host-header allowlist (DNS-rebinding defence).";
  };

  log_level = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  capture_assistant = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  llm_provider = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  llm_model = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  auth = lib.mkOption {
    type = types.nullOr (types.submodule {
      # Keep unknown keys visible to the module's explicit secret-key
      # assertion, which gives a useful refusal instead of a type error.
      freeformType = types.attrsOf types.anything;
      options = {
        secure_cookie = lib.mkOption {
          type = types.nullOr types.bool;
          default = null;
        };
        root_username = lib.mkOption {
          type = types.nullOr types.str;
          default = null;
        };
      };
    });
    default = null;
    description = ''
      Non-secret auth settings only. Use ageSecret, sopsSecret, or
      environmentFile for bearer tokens, password material, and other
      credentials.
    '';
  };
}
