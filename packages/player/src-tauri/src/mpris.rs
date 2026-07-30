use std::thread;
use std::time::Duration;

use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId, Uri};
use serde_json::{json, Value};
use tauri::Manager;
use tokio::runtime::Builder;

use crate::bridge::bridge::Bridge;

const BUS_NAME_SUFFIX: &str = "nuclear";
const IDENTITY: &str = "Nuclear Music Player";
const DESKTOP_ENTRY: &str = "nuclear-music-player";
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const UNKNOWN_TITLE: &str = "Unknown track";
const UNKNOWN_ARTIST: &str = "Unknown artist";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MprisTrackMetadata {
    pub track_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeControl {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekTo,
}

impl BridgeControl {
    fn method(self) -> &'static str {
        match self {
            Self::Play => "Playback.play",
            Self::Pause => "Playback.pause",
            Self::PlayPause => "Playback.toggle",
            Self::Stop => "Playback.stop",
            Self::Next => "Queue.goToNext",
            Self::Previous => "Queue.goToPrevious",
            Self::SeekTo => "Playback.seekTo",
        }
    }
}

pub fn init_mpris(app_handle: tauri::AppHandle) {
    let bridge = app_handle.state::<Bridge>().inner().clone();

    let spawn_result = thread::Builder::new()
        .name("nuclear-mpris".into())
        .spawn(move || run_mpris_thread(bridge));

    if let Err(err) = spawn_result {
        log::warn!(target: "mpris", "failed to spawn MPRIS thread: {err}");
    }
}

fn run_mpris_thread(bridge: Bridge) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            log::warn!(target: "mpris", "failed to create MPRIS runtime: {err}");
            return;
        }
    };

    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        if let Err(err) = run_mpris_service(bridge).await {
            log::warn!(target: "mpris", "MPRIS service unavailable: {err}");
        }
    });
}

async fn run_mpris_service(bridge: Bridge) -> mpris_server::zbus::Result<()> {
    let player = Player::builder(BUS_NAME_SUFFIX)
        .identity(IDENTITY)
        .desktop_entry(DESKTOP_ENTRY)
        .can_play(true)
        .can_pause(true)
        .can_go_next(true)
        .can_go_previous(true)
        .can_seek(true)
        .can_control(true)
        .build()
        .await?;

    connect_controls(&player, bridge.clone());
    tokio::task::spawn_local(player.run());

    let mut notifications = bridge.subscribe_notifications();
    refresh_player(&player, &bridge).await;

    loop {
        tokio::select! {
            _ = tokio::time::sleep(REFRESH_INTERVAL) => refresh_player(&player, &bridge).await,
            notification = notifications.recv() => {
                if notification.map(|event| matches!(event.subsystem.as_str(), "player" | "playlist" | "mixer" | "options")).unwrap_or(false) {
                    refresh_player(&player, &bridge).await;
                }
            }
        }
    }
}

fn connect_controls(player: &Player, bridge: Bridge) {
    let next_bridge = bridge.clone();
    player.connect_next(move |_| {
        dispatch_control(next_bridge.clone(), BridgeControl::Next, json!({}))
    });

    let previous_bridge = bridge.clone();
    player.connect_previous(move |_| {
        dispatch_control(previous_bridge.clone(), BridgeControl::Previous, json!({}))
    });

    let pause_bridge = bridge.clone();
    player.connect_pause(move |_| {
        dispatch_control(pause_bridge.clone(), BridgeControl::Pause, json!({}))
    });

    let play_pause_bridge = bridge.clone();
    player.connect_play_pause(move |_| {
        dispatch_control(
            play_pause_bridge.clone(),
            BridgeControl::PlayPause,
            json!({}),
        )
    });

    let stop_bridge = bridge.clone();
    player.connect_stop(move |_| {
        dispatch_control(stop_bridge.clone(), BridgeControl::Stop, json!({}))
    });

    let play_bridge = bridge.clone();
    player.connect_play(move |_| {
        dispatch_control(play_bridge.clone(), BridgeControl::Play, json!({}))
    });

    player.connect_set_position(move |_, _, position| {
        let seconds = position.as_micros() as f64 / 1_000_000.0;
        dispatch_control(
            bridge.clone(),
            BridgeControl::SeekTo,
            json!({ "seconds": seconds }),
        );
    });
}

fn dispatch_control(bridge: Bridge, control: BridgeControl, params: Value) {
    tauri::async_runtime::spawn(async move {
        if let Err(err) = bridge.call(control.method(), params).await {
            log::warn!(target: "mpris", "MPRIS control {:?} failed: {err}", control);
        }
    });
}

async fn refresh_player(player: &Player, bridge: &Bridge) {
    match current_metadata(bridge).await {
        Ok(track_metadata) => {
            if let Err(err) = player.set_metadata(to_mpris_metadata(track_metadata)).await {
                log::warn!(target: "mpris", "failed to publish metadata: {err}");
            }
        }
        Err(err) => log::debug!(target: "mpris", "metadata refresh skipped: {err}"),
    }

    match bridge.call("Playback.getState", json!({})).await {
        Ok(state) => {
            let status = to_playback_status(state.get("status").and_then(Value::as_str));
            if let Err(err) = player.set_playback_status(status).await {
                log::warn!(target: "mpris", "failed to publish playback status: {err}");
            }
            if let Some(seek) = state.get("seek").and_then(Value::as_f64) {
                player.set_position(Time::from_micros((seek * 1_000_000.0) as i64));
            }
        }
        Err(err) => log::debug!(target: "mpris", "playback refresh skipped: {err}"),
    }
}

async fn current_metadata(bridge: &Bridge) -> Result<MprisTrackMetadata, String> {
    let item = bridge
        .call("Queue.getCurrentItem", json!({}))
        .await
        .map_err(|err| err.to_string())?;
    metadata_from_queue_item(&item)
}

pub fn metadata_from_queue_item(item: &Value) -> Result<MprisTrackMetadata, String> {
    let item_object = item
        .as_object()
        .ok_or_else(|| "current queue item is empty".to_string())?;
    let track = item_object
        .get("track")
        .and_then(Value::as_object)
        .ok_or_else(|| "current queue item has no track".to_string())?;

    let title = track
        .get("title")
        .and_then(non_empty_string)
        .unwrap_or_else(|| UNKNOWN_TITLE.to_string());
    let artists = track
        .get("artists")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|artist| non_empty_string(artist.get("name")?))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![UNKNOWN_ARTIST.to_string()]);

    let album = track
        .get("album")
        .and_then(|album| non_empty_string(album.get("title")?));
    let art_url = item_object
        .get("track")
        .and_then(artwork_url)
        .or_else(|| track.get("album").and_then(artwork_url));
    let duration_ms = track.get("durationMs").and_then(Value::as_i64);
    let track_id = format!(
        "/org/mpris/MediaPlayer2/Track/{}",
        sanitize_track_id(
            item_object
                .get("id")
                .and_then(non_empty_string)
                .unwrap_or_else(|| title.clone())
        )
    );

    Ok(MprisTrackMetadata {
        track_id,
        title,
        artists,
        album,
        art_url,
        duration_ms,
    })
}

fn non_empty_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn artwork_url(container: &Value) -> Option<String> {
    container
        .get("artwork")?
        .get("items")?
        .as_array()?
        .iter()
        .find_map(|artwork| non_empty_string(artwork.get("url")?))
}

fn sanitize_track_id(value: String) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "current".to_string()
    } else {
        sanitized
    }
}

pub fn to_playback_status(status: Option<&str>) -> PlaybackStatus {
    match status {
        Some("playing") => PlaybackStatus::Playing,
        Some("paused") => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    }
}

pub fn to_mpris_metadata(track: MprisTrackMetadata) -> Metadata {
    let mut metadata = Metadata::new();
    metadata.set_trackid(TrackId::try_from(track.track_id).ok());
    metadata.set_title(Some(track.title));
    metadata.set_artist(Some(track.artists));
    metadata.set_album(track.album);
    if let Some(art_url) = track.art_url {
        metadata.set_art_url(Some(Uri::from(art_url.as_str())));
    }
    if let Some(duration_ms) = track.duration_ms {
        metadata.set_length(Some(Time::from_millis(duration_ms)));
    }
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_complete_queue_item_to_mpris_metadata() {
        let item = json!({
            "id": "queue item/1",
            "track": {
                "title": "Paranoid Android",
                "artists": [{ "name": "Radiohead" }],
                "album": { "title": "OK Computer", "artwork": { "items": [{ "url": "https://example.com/album.jpg" }] } },
                "durationMs": 383000,
                "artwork": { "items": [{ "url": "https://example.com/track.jpg" }] }
            }
        });

        let metadata = metadata_from_queue_item(&item).expect("metadata");

        assert_eq!(metadata.title, "Paranoid Android");
        assert_eq!(metadata.artists, vec!["Radiohead"]);
        assert_eq!(metadata.album, Some("OK Computer".to_string()));
        assert_eq!(
            metadata.art_url,
            Some("https://example.com/track.jpg".to_string())
        );
        assert_eq!(metadata.duration_ms, Some(383000));
        assert_eq!(
            metadata.track_id,
            "/org/mpris/MediaPlayer2/Track/queue_item_1"
        );
    }

    #[test]
    fn uses_fallbacks_for_incomplete_metadata() {
        let item = json!({ "id": "", "track": { "title": "", "artists": [] } });

        let metadata = metadata_from_queue_item(&item).expect("metadata");

        assert_eq!(metadata.title, UNKNOWN_TITLE);
        assert_eq!(metadata.artists, vec![UNKNOWN_ARTIST]);
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.art_url, None);
        assert_eq!(
            metadata.track_id,
            "/org/mpris/MediaPlayer2/Track/Unknown_track"
        );
    }

    #[test]
    fn maps_playback_statuses() {
        assert_eq!(to_playback_status(Some("playing")), PlaybackStatus::Playing);
        assert_eq!(to_playback_status(Some("paused")), PlaybackStatus::Paused);
        assert_eq!(to_playback_status(Some("stopped")), PlaybackStatus::Stopped);
        assert_eq!(to_playback_status(None), PlaybackStatus::Stopped);
    }

    #[test]
    fn maps_controls_to_existing_bridge_methods() {
        assert_eq!(BridgeControl::Play.method(), "Playback.play");
        assert_eq!(BridgeControl::Pause.method(), "Playback.pause");
        assert_eq!(BridgeControl::PlayPause.method(), "Playback.toggle");
        assert_eq!(BridgeControl::Stop.method(), "Playback.stop");
        assert_eq!(BridgeControl::Next.method(), "Queue.goToNext");
        assert_eq!(BridgeControl::Previous.method(), "Queue.goToPrevious");
        assert_eq!(BridgeControl::SeekTo.method(), "Playback.seekTo");
    }
}
