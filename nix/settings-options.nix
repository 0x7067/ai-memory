# Typed `services.ai-memory.settings` options for common non-secret keys.
#
# The settings submodule also sets `freeformType = pkgs.formats.toml.type`, so
# any other `config.toml` key can be set without listing it here. Keep this
# file small: only type keys we want docs/validation for. Nested schema
# sections (decay, auto_improve, dream, …) stay freeform to avoid drifting
# against the Rust config.
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

  server_url = lib.mkOption {
    type = types.nullOr types.str;
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

  llm_base_url = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  llm_headers = lib.mkOption {
    type = types.nullOr (types.listOf types.str);
    default = null;
    description = ''
      Extra HTTP headers for the LLM provider (`Header: value` strings).

      Warning: values set here land in the generated `config.toml` Nix store
      path, which is world-readable. Do not put API keys or other secrets in
      `llm_headers`; pass those via `ageSecret` / `sopsSecret` /
      `environmentFile` (for example `AI_MEMORY_LLM_HEADERS`) instead.
    '';
  };

  embedding_provider = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  embedding_model = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  auth = lib.mkOption {
    type = types.nullOr (types.submodule {
      # Freeform so a mistaken secret key reaches the module assertion
      # ("settings.auth must not contain secrets") instead of a generic
      # "option does not exist" type error.
      freeformType = types.attrsOf types.str;
      options = {
        secure_cookie = lib.mkOption { type = types.nullOr types.bool; default = null; };
        root_username = lib.mkOption { type = types.nullOr types.str; default = null; };
        root_issuer = lib.mkOption { type = types.nullOr types.str; default = null; };
        root_subject = lib.mkOption { type = types.nullOr types.str; default = null; };
        root_email = lib.mkOption { type = types.nullOr types.str; default = null; };
        trusted_proxy_cidrs = lib.mkOption {
          type = types.nullOr (types.listOf types.str);
          default = null;
        };
      };
    });
    default = null;
    description = ''
      Non-secret auth settings only. Never put bearer_token, token_pepper,
      initial_root_password, recovery_token, or actor_proxy_bearer_token here —
      use ageSecret / sopsSecret / environmentFile instead.
    '';
  };
}
