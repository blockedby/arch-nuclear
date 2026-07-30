# Plan: Arch Nuclear MPRIS2/KDE Connect PR (#2)

## Task intake
- Goal: expose Linux MPRIS2 now-playing metadata and basic playback controls so KDE Connect/playerctl can observe and control Nuclear.
- In scope: Linux-only Rust service or frontend+Rust bridge; title/artist/album/artwork metadata with fallbacks; playback status; basic controls mapped to existing playback actions; practical Rust/TS tests; explicit live DBus/KDE verification limitation if unavailable.
- Out of scope: upstream/nukeop PRs, merges, host installs, destructive cleanup, broad playback/plugin refactors, overclaiming KDE/Wayland/DBus verification.
- Done state: branch pushed to origin, PR to blockedby/arch-nuclear master opened with issue #2 linkage, fresh local evidence recorded, root report written.
- Blocking unknowns: live session bus/KDE Connect availability may be absent; should be waived with playerctl/manual next steps if not available.

## Repo orientation
- Root guidance read: `AGENTS.md`, `README.md`; no relevant child `AGENTS.md` exists under player/Rust areas.
- Project shape: Tauri app in `packages/player`; Rust backend under `packages/player/src-tauri/src`; frontend stores/services under `packages/player/src`.
- Existing bridge: Rust `bridge::Bridge` calls frontend methods via `bridge:request`; frontend dispatches plugin SDK API in `packages/player/src/services/bridge/bridgeDispatcher.ts`.
- Current playback API: `Playback.getState/play/pause/stop/toggle/seekTo`, Queue API `getCurrentItem/goToNext/goToPrevious` available through plugin SDK bridge.
- Notifications: frontend notifies Rust for player/playlist/mixer/options changes in `bridgeNotifier.ts`; Rust handles notifications in `bridge::Bridge` for listeners.
- Verification commands: `cd packages/player/src-tauri && cargo test`; if TS touched, `pnpm --filter @nuclearplayer/player test -- <target>` or typecheck as practical.

## Reuse discovery
- Reuse `Bridge::call` and bridge notifications rather than creating a second frontend command path.
- Reuse `Playback.getState`, `Queue.getCurrentItem`, `Queue.goToNext`, `Queue.goToPrevious` and playback host methods.
- Follow existing Rust service startup patterns in `http_api`, `mpd`, `mcp`, `discord`, registered from `lib.rs` during Tauri setup.
- Reuse serde_json data extraction for external bridge payloads; keep Linux-specific DBus dependencies target-gated.

## Missing pieces
- Linux MPRIS service module with metadata conversion, playback status conversion, control handlers, DBus registration, and bridge notification refresh.
- Startup wiring in Tauri setup guarded to Linux so non-Linux builds stay unaffected.
- Tests for metadata fallback/status conversion/control dispatch mapping where practical.
- PR body and reports documenting live desktop verification waiver/steps if no session bus.

## Design decisions / quality gates
- Backend/service contract: expose only MPRIS2-compatible fields and preserve existing bridge API contracts; no persisted data or secrets.
- Runtime readiness: initialize only on Linux; tolerate missing DBus/session gracefully with log warning rather than app startup failure.
- Frontend bridge: only touch frontend if existing bridge API cannot provide needed data; prefer Rust-only implementation using existing frontend bridge methods.

## Plan tasks

### Task 1: Linux MPRIS2 service and tests
Goal:
- Implement Linux MPRIS2 DBus service exposing metadata/status and controls via existing bridge.
Boundary:
- System area: Rust backend service/runtime integration.
- Primary verification: `cd packages/player/src-tauri && cargo test` with targeted module tests.
Existing pattern / reuse:
- `packages/player/src-tauri/src/bridge`, `http_api/actions.rs`, `http_api/routes.rs`, `lib.rs` startup wiring.
Missing change:
- Add Linux-gated MPRIS module/dependency, metadata/status conversion with fallbacks, control methods, notifications refresh.
Scope / likely files:
- `packages/player/src-tauri/Cargo.toml`, `src/lib.rs`, new `src/mpris.rs` or `src/mpris/*`; maybe `bridge/types.rs` if needed.
Acceptance criteria:
- MPRIS metadata maps track title/artist/album/artwork where available and fallbacks when missing.
- Playback status maps playing/paused/stopped.
- Basic controls call existing bridge actions: play, pause, stop/toggle as applicable, next, previous, seek if implemented by MPRIS interface.
- Missing session bus does not crash app startup.
Test plan:
- Positive: unit tests for metadata/status conversion and control method-to-bridge method mapping.
- Edge: incomplete/empty queue item yields sensible defaults/no panic; artwork absent omitted or empty per MPRIS-compatible behavior.
- Runtime: cargo test; if session bus/playerctl available, manual `playerctl -p nuclear metadata/status` smoke.
Dependencies: none.
Executor: aad-implementer.
Report path: `docs/plans/2026-06-02-mpris2-now-playing/reports/aad-implementer-mpris2.md`.

### Task 2: Owner final verification and PR
Goal:
- Verify integrated branch, push, open PR to blockedby/arch-nuclear master only, write root report.
Dependencies: Task 1.
Executor: slice owner, optionally acceptance auditor.

## Dependency graph
- Wave 1: Task 1 delegated to aad-implementer.
- Wave 2: Owner review, fresh verification, optional audit, push/open PR.

## Execution ledger
- 2026-06-02: Worktree created from `origin/master`; task package and plan created.


## Execution ledger update
- 2026-06-02: Implementation completed directly by slice owner because nested `aad-implementer` dispatch was blocked by subagent depth. Added Linux-gated `mpris` Rust module using `mpris-server`, startup wiring in `lib.rs`, and tests for metadata fallback/status/control mapping.
- 2026-06-02: Verification passed: `cargo test mpris -- --nocapture` and full `cargo test -- --nocapture` from `packages/player/src-tauri` with disposable `../dist/index.html` test fixture due embedded frontend assets requirement. Live playerctl/KDE Connect verification not run; manual steps recorded in `verification/local.md`.

## Acceptance verification matrix
- AC: MPRIS exposes track title/artist/album/artwork and fallbacks.
  - Covered by: `mpris::tests::maps_complete_queue_item_to_mpris_metadata`, `mpris::tests::uses_fallbacks_for_incomplete_metadata`.
  - Result: passed.
- AC: MPRIS exposes playback status and basic controls mapped to existing playback actions.
  - Covered by: `mpris::tests::maps_playback_statuses`, `mpris::tests::maps_controls_to_existing_bridge_methods`; code connects MPRIS callbacks to `Playback.*` and `Queue.*` bridge calls.
  - Result: passed.
- AC: Local tests/cargo evidence cover conversion/metadata/control paths where practical.
  - Covered by: `cargo test mpris -- --nocapture` and `cargo test -- --nocapture`.
  - Result: passed.
- AC: Live desktop verification is provided or explicitly waived/limited.
  - Covered by: `verification/local.md` waiver; `playerctl` unavailable and no live app session.
  - Result: waived with manual next steps.

- 2026-06-02: Branch pushed and PR opened: https://github.com/blockedby/arch-nuclear/pull/7.
