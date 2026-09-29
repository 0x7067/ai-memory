# Shared systemd hardening for ai-memory (NixOS module + FHS units).
#
# Keep packaging/systemd/ai-memory.service and ai-memory-user.service aligned
# with these keys; scripts/check-native-packaging.sh asserts the system unit
# contains every entry below.
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

  # User-unit sandbox: same confinement where %h paths must stay writable.
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
