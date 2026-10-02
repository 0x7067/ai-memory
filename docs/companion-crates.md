# Optional companion crates and projects

This page records the boundary for feature ideas that are useful around
ai-memory, but should not become core ai-memory surface area. PR #118 and PR
#123 are the historical motivation: both are valid product ideas, but both patch
too much import, chat, UI, and mutation behavior into the core server. The better
shape is optional companion software that orchestrates ai-memory through public
APIs.

ai-memory stays a memory substrate:

- one server binary owns hooks, MCP, the markdown wiki, SQLite indexes, auth,
  admission webhooks, and the built-in read-only browser;
- the wiki remains markdown-in-git as source of truth;
- SQLite remains a derived index;
- writes go through the existing wiki mutation path, admin endpoints, or MCP
  tools;
- the built-in `/web` and `/api/v1` surfaces stay read-oriented.

Companion crates can be richer products. They should integrate through public
HTTP/MCP surfaces instead of patching handlers, routes, or command surfaces into
the core workspace.

## Integration rules for companions

Orchestrators that own their harness lifecycle can use the
[external capture contract](external-lifecycle.md) to suppress native observation
capture per execution while retaining handoff delivery and MCP retrieval.
Producer provenance and retry identity use the existing hook ingestion fields.

Companion projects may:

- call the read-only `/api/v1` endpoints for workspaces, projects, pages,
  search, graph, recent pages, and briefing/overview snapshots;
- call existing MCP tools such as `memory_write_page`, `memory_delete_page`,
  `memory_read_page`, and `memory_query` when running as an agent/client;
- call existing admin endpoints such as `/admin/write-page` and
  `/admin/delete-page` when running as an operator-side server process with an
  appropriate bearer token;
- use `--web-ui-dir` to let ai-memory serve an alternate static SPA, as long as
  the SPA still uses public HTTP APIs and does not require in-process plugins;
- run their own LLM prompts, import transforms, queues, confirmation flows,
  UI state, and project-specific policies;
- ship their own CLI binary, web server, Docker image, tests, release cadence,
  and docs.

Companion projects should not:

- become Cargo workspace members of core ai-memory by default;
- add core MCP tools, admin endpoints, or CLI subcommands unless a missing seam
  is independently useful to ai-memory itself;
- write wiki files or SQLite rows directly;
- bypass `AuthLevel::authorize`, admission webhooks, actor attribution, scope
  resolution, or the single-writer store boundary;
- require ai-memory to host arbitrary plugin code in-process.

Companion features should be treated as separate products, not rejected ideas.
They can move faster than core, have their own UX, and carry source-specific or
workflow-specific behavior without widening ai-memory's default install.

If a companion exposes browser writes, it must implement its own server-side
mutation broker. Browsers should talk to the companion; the companion should talk
to ai-memory with a server-held credential bound to the authenticated operator.
That keeps CSRF, confirmation, audit, rate limits, and UI-specific policy outside
the core server. The proposed editor's [browser boundary](#browser-boundary)
details its DB-user-only broker and identity binding.

Read-only companions can start ai-memory with `serve --enable-api` to mount
`/api/v1` without the browser UI. `--enable-web` still implies the same API.
Both modes use the normal machine Bearer or browser-session auth boundary;
writes continue through MCP or authenticated admin routes.

### Proposed team-wiki sync (#986)

A repository-backed team-wiki sync is accepted as a companion shape, not as a
repo-local storage mode in the core server. A contribution should preserve the
prototype's three-way comparison and clone-local state, sync only explicitly
allowed shared page families, default destructive changes to dry-run, report
divergent edits without choosing a winner, and perform every import through
the public MCP write/delete tools. It must never open the wiki directory or
SQLite directly. The core read seam is `/api/v1` in API-only mode; the supported
MCP page arguments are documented in [programmatic memory](programmatic-memory.md).

## `ai-memory-relay`: external lifecycle delivery

[`ai-memory-relay`](../companions/ai-memory-relay) delivers events collected by
an external orchestrator through `/hook/batch`. Its own local queue records events
before sending and retains unacknowledged entries for a later flush. It does not
launch agents, claim handoffs, or open ai-memory's database or wiki.

The orchestrator still maps its events to the native harness payloads and sets
`AI_MEMORY_CAPTURE_OWNER` when launching that harness. The relay uses the native
session identity and derives retry keys from the producer's stable event IDs.
Only the first pending event for each session enters a batch; that session
advances after acknowledgement, even when other sessions are rate-limited.

The package has its own workspace, tests and CLI. Its README defines queue limits,
local data handling and recovery, with an executable test against the real
ai-memory server. Root workspace tests do not run the companion's unit tests.

## `ai-memory-importer`: migration and ingestion companion

This is the companion shape for PR #118. The first implemented companion lives
at [`companions/ai-memory-importer`](../companions/ai-memory-importer) as a
standalone Cargo package with its own `[workspace]`; it is not a member of the
root workspace and is not covered by root `cargo test --workspace`.

### Goal

Import or normalize existing memory corpora without making ai-memory core own
every source format and migration workflow.

Initial source support is intentionally narrow:

- oh-my-claudecode / OMC flat markdown wiki directories.
- generic external-conversation JSON envelopes (`project`, `source`,
  `session_id`, and bounded `role`/`content` messages), replayed through the
  public ordered hook-batch surface. Product-specific export adapters remain
  outside this repository.

Future sources can include:

- Claude Code memory graph exports such as `memory.jsonl` from
  `@modelcontextprotocol/server-memory`;
- Qdrant-backed memory collections, when a user supplies a collection URL and
  schema mapping;
- future one-off importers maintained on the companion's release cadence.

### Validation

Run companion checks explicitly from the repository root:

```bash
cargo fmt --check --manifest-path companions/ai-memory-importer/Cargo.toml
cargo test --manifest-path companions/ai-memory-importer/Cargo.toml
cargo clippy --manifest-path companions/ai-memory-importer/Cargo.toml --all-targets -- -D warnings
```

Root hygiene checks remain separate:

```bash
cargo fmt --check
git diff --check
```

### Product shape

Prefer a separate repository and binary crate, for example:

```text
companions/ai-memory-importer/
├── Cargo.toml
├── src/main.rs
└── README.md
```

It can share Rust libraries later only if those libraries are published with a
stable API and are useful outside ai-memory. It should not need to be a member of
this workspace.

### How it talks to ai-memory

Read and plan:

- use `/api/v1/workspaces`, `/api/v1/projects`, `/api/v1/pages`,
  `/api/v1/search`, and `/api/v1/graph` to inspect the destination;
- default to dry-run, printing planned page writes without mutating ai-memory.

Write:

- write imported or normalized pages through `/admin/write-page` or MCP
  `memory_write_page`;
- do not delete in v1;
- use `memory_query` / `memory_read_page` or `/api/v1/search` / page reads for
  duplicate detection and context checks;
- optionally call `memory_consolidate` or `memory_auto_improve` after import for
  post-import refinement, rather than building that refinement into core;
- for bulk operations, loop over the public single-page operation unless ai-memory
  later adds a generic bulk-mutation seam for its own reasons.

Re-home by kind:

- compute the move/link-rewrite plan in the companion;
- apply moves as normal writes to the new path plus deletes of the old path;
- preserve frontmatter that ai-memory returns through page reads;
- fail closed on collisions, missing pages, or changed source hashes.

### Safety requirements

- Never open ai-memory's SQLite database or wiki directory directly.
- Require an explicit destination workspace/project.
- Preserve only metadata supported by the public write surface (`title`, `kind`,
  `tier`, `tags`, `pinned`, and body) unless a future generic core seam adds
  broader frontmatter support. Do not claim arbitrary frontmatter or author
  preservation in companion imports.
- Carry idempotency keys or source fingerprints in companion-side state so failed
  imports can be resumed safely.
- Validate and sanitize a complete external-conversation envelope before the
  first HTTP request; use a stable imported session identity and per-event
  idempotency keys, and send the dedicated `external-import` wire identity
  rather than impersonating a live coding harness.
- Surface all destructive actions in dry-run output before live mode.
- Treat non-overwrite checks as best-effort unless/until core exposes a generic
  compare-and-write seam; companion v1 re-checks before each write but cannot make
  `/admin/write-page` atomic with that read.
- Keep LLM normalization optional; deterministic import should work with no LLM.
- Keep provider-specific performance tweaks, such as model parameter changes, out
  of importer PRs. If ai-memory core needs a provider bugfix or optimization,
  land it as a small standalone core change.

### Implementation plan

1. Build a read-only planner for one source format and snapshot fixtures.
2. Add dry-run output and collision detection.
3. Add live writes through existing ai-memory public write/delete surfaces.
4. Add optional LLM normalization as a companion-side pass.
5. Add re-home/link-rewrite as a separate subcommand after import is stable.
6. Only after repeated usage, consider whether ai-memory core lacks a small,
   generic API seam; do not start by patching core endpoints.

## `ai-memory-macos`: menu bar wrapper

Self-contained macOS accessory app at
[`companions/ai-memory-macos`](../companions/ai-memory-macos). It is a
**wrapper**, not a data-seam dashboard: it ships the `ai-memory` binary and
`hooks/` tree inside an `.app`, governs the existing LaunchAgent, and opens
`/web`, `ai-memory status`, `config.toml`, the data directory, and logs.

It is not a root workspace member. Build and test it separately:

```bash
./companions/ai-memory-macos/build.sh
swift test --package-path companions/ai-memory-macos
```

### Goal

Give macOS a first-class install that does not require a prior tarball, without
reimplementing status, search, wiki browsing, or config editing in SwiftUI.

### How it talks to ai-memory

- `GET /admin/status` for the menu-bar traffic light and two headline lines
  (version, page/session counts, LLM role).
- Bundled `ai-memory status` / `ai-memory init` via `Process` (the real CLI).
- `launchctl` against `com.github.akitaonrails.ai-memory` and the checked-in
  plist template in `packaging/launchd/`.
- `NSWorkspace` to open `/web`, `config.toml`, the data dir, and logs.

It does not open SQLite or the wiki, does not call writable `/admin` routes, and
does not add MCP tools.

### Data vs bundle

Durable state stays in `~/Library/Application Support/ai-memory` (the binary’s
existing macOS default) and logs under `~/Library/Logs/ai-memory`. Replacing
`/Applications/AI Memory.app` does not rewrite that tree. An optional data-dir
override is written only into the rendered LaunchAgent plist
(`AI_MEMORY_DATA_DIR`), never into the bundle.

### Safety requirements

- Do not silently start the LaunchAgent on first launch; **Install & Start**
  is an explicit click.
- Do not write `AI_MEMORY_AUTH_TOKEN` into the plist. The menu bar’s own HTTP
  client may keep a bearer in the Keychain.
- Do not sandbox the app in a way that blocks `launchctl` or LaunchAgents.

See [`docs/macos.md`](macos.md#scenario-d-menu-bar-app).

## `ai-memory-web-editor`: browser curation/editor companion

PR #123 motivated this companion shape. This fresh design proposal narrows
it to reviewing and correcting stored memory, with implementation split into
small dependent changes.

Status: **undecided**. Maintainer evaluation of this proposal is pending; neither
the editor nor the proposed prerequisites below have upstream design acceptance
by virtue of this document. The existing concern remains: a writable browser
needs auth, concurrency control, confirmation, conflict handling, and audit.
The review should weigh correction of wrong or stale memory against the cost of
maintaining that product alongside the automatic improvement loop.

### Goal

Let an operator inspect the source of a claim, draft a correction, compare it
with the stored revision, and deliberately submit it. Capture, consolidation,
pending writes, and eval gates continue to own the automatic loop. Inspection,
drafting, and editing must work without an LLM.

The MVP does not execute procedures or introduce chat. Its views use only data
available through current authorized reads; absent evidence stays absent. Core
`/web` and `/api/v1` remain read-oriented. Canonical ADRs stay in their repository:
the editor links to their source/revision instead of creating a second decision
record in memory.

### Product shape

The proposed location is `companions/ai-memory-web-editor`, a standalone Rust
package with its own `[workspace]`, backend, browser assets, and integration/E2E
tests. It is not a root Cargo workspace member. It has its own validation and
release cadence and never opens ai-memory's wiki or SQLite directly.

Public requests should use the proposed minimal client SDK, after it has two
real consumers, relay and importer. The SDK and its capability contract are
prerequisites proposed separately, not shipped editor APIs. An older server or
an unavailable capability must leave the dependent feature disabled; the
capability catalogue never grants access to a user or project.

The companion may run beside ai-memory behind a reverse proxy, but owns a
separate browser origin or explicitly isolated path and cookie namespace. It
does not assume ai-memory's human-auth cookies authenticate the companion; the
core [HTTP authentication classes](ARCHITECTURE.md#http-authentication-classes)
still govern every upstream request.

### Browser boundary

The read-only shell already needs this boundary, before any editing is enabled:

- The companion implements its own login, session expiry/revocation, and CSRF
  protection, including login and later state-changing draft/confirmation
  requests. Session cookies are HttpOnly with appropriate Secure/SameSite and
  path settings; origin checks and session-bound CSRF tokens protect mutations.
- Bind the authenticated browser principal to the same server-authenticated
  ai-memory identity. The broker exclusively holds that operator's own DB-user
  API token server-side. Refuse missing, mismatched, or revoked
  identity/credentials; never fall back to a shared credential. Provisioning and
  secure credential storage are part of the shell's proposed design, not an
  existing core login exchange.
- Root/admin flows and admin proposal views/actions are excluded from this MVP.
  Do not accept or store the shared static root bearer: it cannot identify which
  human made a browser request. A DB-user token attached to a `role=root`
  identity still authenticates as `AuthLevel::User`; it cannot authorize
  `/admin/*`. Any future admin views need a separate design proposal.
- The browser receives neither a machine credential nor an upstream bearer.
  Build upstream requests from allowed fields with only the operator's own
  DB-user token. Never act as an actor proxy or configure/use
  `actor_proxy_bearer_token`. Never fabricate or forward `X-Memory-Actor-*` or
  `X-Memory-Skip-Admission-Chain`, or forward browser-supplied `Authorization`.
  The authenticated DB-user identity supplies real attribution and admission
  context.
- Reads also pass through the broker under the operator's credential. Send
  explicit workspace/project pairs on project-scoped calls, preserve current
  per-project access and ownership rules, and reauthorize each request. Drafts, caches, and
  confirmations are isolated by operator, server, workspace, project, and path.
  Changing account or scope must not expose another coordinate's state. Shared
  pages remain shared; `author_id` must not become a page-read filter.
- Honor projects' `open`/`restricted` modes and `read`/`write` grants (`write`
  includes `read`) from the core [per-project access](users.md#per-project-access)
  contract. An explicit request refused for a target or access level must
  preserve the 403 response, never turn it into an empty result. Search, listings,
  and graph reads retain the core's filtering of inaccessible projects.

These requirements preserve `ScopeResolver`, `AuthLevel::authorize`, Wiki
sanitization/admission/attribution, and the single-writer boundary. The companion
may restrict its own targets further; UI filters never replace authorization.

### Proposed delivery sequence

Every stage below is proposed. A stage's unavailable prerequisites must be
delivered and tested before that feature is enabled; this sequence defines no
new shipped endpoint or wire field.

1. Read-only shell. Requires maintainer design evaluation and the proposed
   client SDK with its two consumers. Deliver the browser boundary above and
   browse the existing `/api/v1` and MCP reads. No wiki mutation is enabled.
   Display explicit access refusals as 403, distinct from missing data.
2. Draft, preview, and diff. Requires the shell and proposed bounded
   retained-version reads. Keep a draft with its base revision, supported
   metadata, and exact target. Render untrusted Markdown safely and show a
   reproducible diff. There is no apply action; drafts never write to the wiki.
   History must expose only eligible retained versions under current per-project access,
   without recovering expired or purged content from Git.
3. Confirmed conditional editing. Requires the proposed core patch and
   compare-and-swap (CAS) seam. Confirmation binds the operator, target,
   base revision, and exact patch/diff; a changed draft needs a new confirmation.
   Preserve omitted metadata, distinguish explicit clears, and disallow editing
   scope or authorship. Deletes remain disabled until the proposed conditional
   delete seam has its own target/diff confirmation and tests.
4. Graph and evidence views. Requires conditional editing and proposed
   directed, bounded graph reads plus portable evidence. Show relation
   direction, source revision, and evidence provenance from authorized data.
   Authorize both ends and every hop; inaccessible nodes/edges cannot leak through
   labels, counts, or expansion. ADR navigation follows the canonical reference.
5. Attempts and procedure views. Requires conditional editing and proposed
   attempt/result/revalidation/procedure contracts. Display incomplete
   or failed attempts, result attribution, applicability, and missing evidence
   explicitly. Narrated success is separate from a checker-verified result.
   Procedure steps remain inspection-only. Split these views further if needed
   to keep each implementation small.

### Confirmation and conflict handling

The proposed revision token must cover the latest version and editable stored
metadata, so a metadata-only change also invalidates confirmation. Core must
compare it in the writer transaction and coordinate disk divergence through the
Wiki lock. A companion read followed by an unconditional write is insufficient;
do not enable apply on servers lacking conditional writes. CAS does not promise
coordination with arbitrary file editors that ignore the process's locking.

On a stale token, retain the draft and its base, fetch the currently authorized
revision, and show the conflict. Reconcile explicitly and obtain a new
confirmation; never retry as an unconditional overwrite. Admission remains in
the normal Wiki path. Display transformed content and refusals according to the
server's authoritative result. Webhook failures/timeouts follow the configured
`failure_policy`: `reject` aborts the write, while `ignore` logs the failure and
continues. Only report committed content after server confirmation. Direct
admission does not create a pending approval; [auto-improvement pending
writes](auto-improvement-loop.md#pending-review-ux) are a separate flow excluded
from this MVP. Preserve drafts on rejection or uncertain delivery, and reconcile
delivery status before retrying. Record operator, scope, base, confirmed diff,
and outcome in the companion audit without retaining secrets.

### Required validation before implementation acceptance

Each implementing change must add adversarial integration/browser tests with a
legitimate control and prove failure with its guard removed. These are required
tests, not coverage claimed by this documentation-only proposal:

- XSS: hostile Markdown/HTML, link schemes, graph labels, and diffs cannot run
  scripts or expose credentials; ordinary content still renders.
- CSRF/session: missing or mismatched tokens, cross-origin requests, and expired
  sessions cannot change drafts or apply edits; a valid same-session request can.
- Two users/two projects: forged actor/skip headers, token identity mismatch,
  root/proxy credentials, account switching, and foreign draft/version IDs fail
  closed. Explicit restricted-project requests with insufficient or revoked
  grants return 403; authorized shared-page reads and operator-attributed writes
  succeed as controls.
- Stale CAS: concurrent body or metadata changes refuse apply and preserve the
  draft/current variant; an explicitly reconfirmed current revision succeeds.
- Admission/recovery: transform, webhook failures/timeouts under both `reject`
  and `ignore`, and interrupted writes keep truthful UI state and attribution.
  Reject-policy failures leave the target unchanged; ignore-policy failures
  allow the normal write. Prove core disk/SQL rollback and recovery, with an
  admitted DB-user write as control. Direct-write tests must not expect pending
  approval.

The companion's standalone workspace needs its own fmt/clippy/tests plus real
broker/browser checks against a temporary server. Implemented boundaries must
be added to the [security inventory](security-boundaries.md) with enforcing code
and guard-off evidence in the same change; proposed seams keep their own core
regressions. Root workspace tests alone cannot validate the companion.

## Two kinds of companion: data-seam vs. independent hook consumer

`ai-memory-importer` and `ai-memory-web-editor` are *data-seam* companions: they
talk to ai-memory's public HTTP/MCP surfaces and build on the data it stores.
Not every adjacent tool is that shape.

**Working-tree coordination is out of core, and is an *independent hook
consumer*, not a data-seam companion** (decided in #620). Several agent sessions
sharing one checkout collide over the single git index — one session's
`git add .` sweeps up another's staged work, a `--fix` run rewrites an unclaimed
tree — and the natural instinct is to build the guard on ai-memory's captured
`PreToolUse`/`PostToolUse` signal. That does not work, for two deliberate
reasons:

- **ai-memory cannot block a tool action.** The `/hook` path is capture-only and
  fire-and-forget (it returns `202`/`429`, never allow/deny/ask, and processes
  after responding). Hooks that await a REST round-trip can deadlock the engine
  (agentmemory #221) — so there is no synchronous veto channel back to the
  harness, by design.
- **ai-memory does not retain the file paths.** For closed-tool agents the
  stored observation is reduced to a `tool_family` label plus outcome; raw
  arguments, paths, and tool names are extracted only transiently for
  denylist matching, then dropped (`CaptureDecision` "never retains raw
  arguments, paths, or arbitrary tool names"). A consumer can see *that* a file
  op happened in a session, never *which file*.

Reversing either — persisting paths, or adding a blocking hook — trades away a
privacy/bounding invariant or the anti-deadlock invariant. Both stay.

So a working-tree coordinator installs its **own** `PreToolUse` hook alongside
ai-memory's, reads the raw `tool_input`, and arbitrates synchronously in its own
process with its own ephemeral ownership state (a lock file or small store —
never the wiki or SQLite). It is a *sibling on the same hook event*, not a
seam-consumer. It may still live under `companions/` for discoverability, but it
depends on the harness's hook mechanism, not on ai-memory's surfaces. Scope it
to the shared-index / file-ownership class; stale-tree builds and host
saturation are build/CI-orchestration, a separate problem.

## When to move a seam into core

A companion may reveal a missing primitive that belongs in ai-memory. Move only
small, generic seams into core, and only after the companion proves the need.

Good core candidates:

- a read-only API field needed by several clients;
- a narrowly-scoped mutation endpoint that is equivalent to an existing MCP tool;
- a capability check or scope-resolution helper that prevents duplicated security
  logic.

Poor core candidates:

- source-specific import parsers;
- UI workflows;
- LLM chat prompts for editing;
- project-specific scoring, pruning, or normalization policies;
- companion-only admin commands.

This keeps ai-memory stable while still allowing richer tools to grow around it.
