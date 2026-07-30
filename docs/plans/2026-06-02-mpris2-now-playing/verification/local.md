# Local verification

Environment notes:
- `DBUS_SESSION_BUS_ADDRESS` exists, but `playerctl` is not installed and the Tauri app was not launched in a live KDE/Wayland session during this run.
- `cargo test` requires `packages/player/dist/` because `http_api/frontend.rs` embeds frontend assets. For Rust verification only, a disposable minimal `packages/player/dist/index.html` was created before commands and removed afterward; it is not committed.

## Commands

```bash
cd packages/player/src-tauri
cargo test mpris -- --nocapture
```

Result: passed. Evidence: 4 MPRIS tests passed (`maps_complete_queue_item_to_mpris_metadata`, `uses_fallbacks_for_incomplete_metadata`, `maps_playback_statuses`, `maps_controls_to_existing_bridge_methods`). Existing warnings: `mpd::protocol::Password` and `ACK_ERROR_NO_EXIST` dead-code warnings.

```bash
cd packages/player/src-tauri
cargo test -- --nocapture
```

Result: passed. Evidence: 28 lib tests passed, 0 main tests, 0 doc tests. Existing warnings: `mpd::protocol::Password` and `ACK_ERROR_NO_EXIST` dead-code warnings.

## Live desktop verification

Not run. `playerctl` is unavailable in this environment and no live Nuclear/KDE Connect session was started. Recommended manual check after PR build/run on Linux desktop:

```bash
playerctl -p nuclear metadata
playerctl -p nuclear status
playerctl -p nuclear play-pause
playerctl -p nuclear next
playerctl -p nuclear previous
```
