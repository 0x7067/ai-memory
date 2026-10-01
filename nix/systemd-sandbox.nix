# Shared systemd hardening for the NixOS ai-memory module.
#
# Applied by `nix/nixos-module.nix`. Packaged FHS units under
# `packaging/systemd/` keep their existing lighter hardening for now;
# aligning those units with this sandbox is a follow-up (needs a real unit
# start test — `systemd-analyze verify` alone is not enough).
{ lib, ... }:

{
  # Full sandbox for the system unit (StateDirectory + /var/lib/ai-memory).
  aiMemorySystemSandbox = {
    CapabilityBoundingSet = [ ];
    AmbientCapabilities = [ ];
    MemoryDenyWriteExecute = true;
    RestrictAddressFamilies = [
      "AF_UNIX"
      "AF_INET"
      "AF_INET6"
    ];
    RestrictNamespaces = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    LockPersonality = true;
    PrivateDevices = true;
    RemoveIPC = true;
    ProtectKernelTunables = true;
    ProtectKernelModules = true;
    ProtectKernelLogs = true;
    ProtectControlGroups = true;
    ProtectClock = true;
    ProtectHostname = true;
    SystemCallArchitectures = "native";
    UMask = "0077";
    NoNewPrivileges = true;
    PrivateTmp = true;
    ProtectHome = true;
    ProtectSystem = "strict";
  };

  # User-unit sandbox (reserved for a future FHS user-unit hardening PR).
  # Same confinement keys less ProtectHome/ProtectSystem, which conflict
  # with %h data paths.
  aiMemoryUserSandbox = {
    CapabilityBoundingSet = [ ];
    AmbientCapabilities = [ ];
    MemoryDenyWriteExecute = true;
    RestrictAddressFamilies = [
      "AF_UNIX"
      "AF_INET"
      "AF_INET6"
    ];
    RestrictNamespaces = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    LockPersonality = true;
    PrivateDevices = true;
    RemoveIPC = true;
    ProtectKernelTunables = true;
    ProtectKernelModules = true;
    ProtectKernelLogs = true;
    ProtectControlGroups = true;
    ProtectClock = true;
    ProtectHostname = true;
    SystemCallArchitectures = "native";
    UMask = "0077";
    NoNewPrivileges = true;
    PrivateTmp = true;
  };
}
