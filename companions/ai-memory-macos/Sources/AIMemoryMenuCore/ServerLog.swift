import Foundation

/// Reads the tail of the bundled server's log so the menu extra can show the
/// reason the LaunchAgent exited — for example a port it cannot bind — instead
/// of only a red status item.
///
/// The server writes its fatal error last, the way `anyhow` renders it:
///
///     Error: <context>
///
///     Caused by:
///         <cause>
public enum ServerLog {
    /// Returns the trailing `Error:` block from `url`, or `nil` when the file
    /// is missing/unreadable or holds no `Error:` line.
    ///
    /// `maxBytes` bounds the read so an unbounded log cannot be pulled into
    /// memory just to show one message.
    public static func recentError(at url: URL, maxBytes: Int = 64 * 1024) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }
        let size = (try? handle.seekToEnd()) ?? 0
        let offset = size > UInt64(maxBytes) ? size - UInt64(maxBytes) : 0
        try? handle.seek(toOffset: offset)
        guard let data = try? handle.readToEnd() else { return nil }
        // The tail can start mid-scalar; lossy decoding keeps the visible text.
        return extractError(from: String(decoding: data, as: UTF8.self))
    }

    /// Last `Error:` line and everything after it, trimmed. Split out so a test
    /// can exercise the parsing without the filesystem.
    static func extractError(from log: String) -> String? {
        let lines = log.components(separatedBy: "\n")
        guard let start = lines.lastIndex(where: { $0.hasPrefix("Error:") }) else {
            return nil
        }
        let block = lines[start...]
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return block.isEmpty ? nil : block
    }
}

/// Picks the message the menu extra shows when the server is unreachable.
public enum StartFailure {
    /// Prefers the server's own fatal log line over the transport/decode error
    /// the health poll produced: `Address already in use` explains the failure,
    /// `Could not read /admin/status` does not.
    ///
    /// Only when the agent is installed but stopped — the exit-loop the bind
    /// failure produces. While it is `.running` (or never installed) a log line
    /// left over from an earlier attempt would be misreported over the live
    /// health error.
    public static func message(health: String, launchd: LaunchdState, logURL: URL) -> String {
        guard launchd == .stopped,
              let logged = ServerLog.recentError(at: logURL)
        else { return health }
        return logged
    }
}
