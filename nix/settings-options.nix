# Typed `services.ai-memory.settings` options mirroring config.toml.
# Secrets belong in age/sops/environmentFile, not here.
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

  tcp_keepalive_secs = lib.mkOption {
    type = types.nullOr types.int;
    default = null;
  };

  server_url = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  base_path = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  hook_rate_per_sec = lib.mkOption {
    type = types.nullOr types.float;
    default = null;
  };

  hook_rate_burst = lib.mkOption {
    type = types.nullOr types.float;
    default = null;
  };

  capture_assistant = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  consolidate_on_session_end = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  backfill_on_start = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  run_autowire = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  strip_root_combinators = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  gemini_safe_schemas = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  reranker = lib.mkOption {
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

  llm_compat_strict = lib.mkOption {
    type = types.nullOr types.bool;
    default = null;
  };

  llm_timeout_secs = lib.mkOption {
    type = types.nullOr types.int;
    default = null;
  };

  llm_reasoning_effort = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  llm_headers = lib.mkOption {
    type = types.nullOr (types.listOf types.str);
    default = null;
  };

  embedding_provider = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  embedding_model = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  embedding_dim = lib.mkOption {
    type = types.nullOr types.int;
    default = null;
  };

  embedding_base_url = lib.mkOption {
    type = types.nullOr types.str;
    default = null;
  };

  cors_allow_origins = lib.mkOption {
    type = types.nullOr (types.listOf types.str);
    default = null;
  };

  decay = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        lambda = lib.mkOption { type = types.nullOr types.float; default = null; };
        sigma = lib.mkOption { type = types.nullOr types.float; default = null; };
        mu = lib.mkOption { type = types.nullOr types.float; default = null; };
        salience_default = lib.mkOption { type = types.nullOr types.float; default = null; };
        cold_threshold = lib.mkOption { type = types.nullOr types.float; default = null; };
        hard_delete_after_days = lib.mkOption { type = types.nullOr types.int; default = null; };
        breadth_weight = lib.mkOption { type = types.nullOr types.float; default = null; };
        observation_retention_days = lib.mkOption { type = types.nullOr types.int; default = null; };
        observation_prune_batch = lib.mkOption { type = types.nullOr types.int; default = null; };
        compact_cold_episodic = lib.mkOption { type = types.nullOr types.bool; default = null; };
        dedup_cold_clusters = lib.mkOption { type = types.nullOr types.bool; default = null; };
        dedup_min_pts = lib.mkOption { type = types.nullOr types.int; default = null; };
        dedup_max_eps = lib.mkOption { type = types.nullOr types.float; default = null; };
        half_life_days = lib.mkOption {
          type = types.nullOr (types.submodule {
            options = {
              working = lib.mkOption { type = types.nullOr types.float; default = null; };
              episodic = lib.mkOption { type = types.nullOr types.float; default = null; };
              semantic = lib.mkOption { type = types.nullOr types.float; default = null; };
              procedural = lib.mkOption { type = types.nullOr types.float; default = null; };
            };
          });
          default = null;
        };
      };
    });
    default = null;
  };

  sanitize = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        extra_patterns = lib.mkOption {
          type = types.nullOr (types.listOf types.str);
          default = null;
        };
        allowlist = lib.mkOption {
          type = types.nullOr (types.listOf types.str);
          default = null;
        };
      };
    });
    default = null;
  };

  slots = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        per_user = lib.mkOption { type = types.nullOr types.bool; default = null; };
      };
    });
    default = null;
  };

  consolidation = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        max_input_tokens = lib.mkOption { type = types.nullOr types.int; default = null; };
        max_output_tokens = lib.mkOption { type = types.nullOr types.int; default = null; };
      };
    });
    default = null;
  };

  auto_improve = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        require_approval = lib.mkOption { type = types.nullOr types.bool; default = null; };
        on_session_end = lib.mkOption { type = types.nullOr types.bool; default = null; };
        min_observations = lib.mkOption { type = types.nullOr types.int; default = null; };
        min_session_duration_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        min_confidence = lib.mkOption { type = types.nullOr types.float; default = null; };
        max_input_tokens = lib.mkOption { type = types.nullOr types.int; default = null; };
        max_proposals_per_run = lib.mkOption { type = types.nullOr types.int; default = null; };
        eval = lib.mkOption {
          type = types.nullOr (types.submodule {
            options = {
              enabled = lib.mkOption { type = types.nullOr types.bool; default = null; };
              command = lib.mkOption { type = types.nullOr types.str; default = null; };
              timeout_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
              targets = lib.mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
              };
              min_delta = lib.mkOption { type = types.nullOr types.float; default = null; };
            };
          });
          default = null;
        };
        scheduler = lib.mkOption {
          type = types.nullOr (types.submodule {
            options = {
              enabled = lib.mkOption { type = types.nullOr types.bool; default = null; };
              interval_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
              max_sessions_per_tick = lib.mkOption { type = types.nullOr types.int; default = null; };
              min_session_age_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
              experience_every_sessions = lib.mkOption { type = types.nullOr types.int; default = null; };
              experience_sessions = lib.mkOption { type = types.nullOr types.int; default = null; };
            };
          });
          default = null;
        };
      };
    });
    default = null;
  };

  maintenance = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        enabled = lib.mkOption { type = types.nullOr types.bool; default = null; };
        forget_sweep_interval_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        lint_interval_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        embedding_backfill_interval_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
      };
    });
    default = null;
  };

  dream = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        enabled = lib.mkOption { type = types.nullOr types.bool; default = null; };
        interval_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        idle_window_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        min_pts = lib.mkOption { type = types.nullOr types.int; default = null; };
        max_eps = lib.mkOption { type = types.nullOr types.float; default = null; };
        max_clusters_per_run = lib.mkOption { type = types.nullOr types.int; default = null; };
        min_cold_pages = lib.mkOption { type = types.nullOr types.int; default = null; };
      };
    });
    default = null;
  };

  retrieval = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        query_intent = lib.mkOption { type = types.nullOr types.bool; default = null; };
        session_recall_bonus = lib.mkOption { type = types.nullOr types.float; default = null; };
        abstract_vectors = lib.mkOption { type = types.nullOr types.bool; default = null; };
        belief_authority_weight = lib.mkOption { type = types.nullOr types.float; default = null; };
      };
    });
    default = null;
  };

  routing = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        mid_session = lib.mkOption { type = types.nullOr types.str; default = null; };
      };
    });
    default = null;
  };

  auto_scope = lib.mkOption {
    type = types.nullOr (types.submodule {
      options = {
        mode = lib.mkOption { type = types.nullOr types.str; default = null; };
        session_ttl_secs = lib.mkOption { type = types.nullOr types.int; default = null; };
        max_entries = lib.mkOption { type = types.nullOr types.int; default = null; };
      };
    });
    default = null;
  };

  auth = lib.mkOption {
    type = types.nullOr (types.submodule {
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
