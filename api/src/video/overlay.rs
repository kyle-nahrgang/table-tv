//! Match overlay: separate text files per piece. FFmpeg drawtext with textfile+reload for each.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::db::pool_match::{MatchPlayer, MatchType, Rating};
use crate::db::Db;
use crate::video::rtmp;

/// Overlay directory.
const OVERLAY_DIR: &str = "data";

/// Overlay piece identifiers.
pub const OVERLAY_P1_NAME: &str = "p1name";
pub const OVERLAY_P1_RATING: &str = "p1rating";
pub const OVERLAY_P2_NAME: &str = "p2name";
pub const OVERLAY_P2_RATING: &str = "p2rating";
pub const OVERLAY_RACE_TO: &str = "raceto";
pub const OVERLAY_RACE_TO2: &str = "raceto2";
pub const OVERLAY_SCORE1: &str = "score1";
pub const OVERLAY_SCORE2: &str = "score2";

fn overlay_name_for_camera(camera_name: &str) -> String {
    let sanitized: String = camera_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

/// Overlay text file path for a camera piece.
pub fn overlay_path_for_camera_piece(camera_name: &str, piece: &str) -> PathBuf {
    let name = overlay_name_for_camera(camera_name);
    std::path::Path::new(OVERLAY_DIR).join(format!("rtmp-overlay-{}-{}.txt", name, piece))
}

/// Overlay path for a camera (returns p1name path; used by stream API).
pub fn overlay_path_for_camera(camera_name: &str) -> PathBuf {
    overlay_path_for_camera_piece(camera_name, OVERLAY_P1_NAME)
}

/// Resolved overlay paths for FFmpeg. All paths must exist before use.
#[derive(Clone)]
pub struct OverlayPaths {
    pub p1name: String,
    pub p1rating: String,
    pub p2name: String,
    pub p2rating: String,
    pub raceto: String,
    pub raceto2: String,
    pub score1: String,
    pub score2: String,
}

/// Resolve all overlay paths for a camera. Fails if any file does not exist.
pub fn resolve_overlay_paths_for_camera(camera_name: &str) -> Result<OverlayPaths, String> {
    Ok(OverlayPaths {
        p1name: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_P1_NAME))?,
        p1rating: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_P1_RATING))?,
        p2name: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_P2_NAME))?,
        p2rating: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_P2_RATING))?,
        raceto: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_RACE_TO))?,
        raceto2: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_RACE_TO2))?,
        score1: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_SCORE1))?,
        score2: rtmp::resolve_overlay_path(&overlay_path_for_camera_piece(camera_name, OVERLAY_SCORE2))?,
    })
}


/// Overlay data for an active match. Displayed at bottom of stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOverlay {
    pub player_one: OverlayPlayer,
    pub player_two: OverlayPlayer,
    pub is_practice: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayPlayer {
    pub name: String,
    pub rating: Option<String>,
    pub games_won: u8,
    pub race_to: u8,
}

impl OverlayPlayer {
    pub fn from_match_player(p: &MatchPlayer) -> Self {
        let rating = p.rating.as_ref().map(|r| match r {
            Rating::Apa(v) => format!("APA {}", v),
            Rating::Fargo(v) => format!("Fargo {}", v),
        });
        Self {
            name: p.name.clone(),
            rating,
            games_won: p.games_won,
            race_to: p.race_to,
        }
    }
}

/// Shared overlay state. Updated by pool_match handlers when match changes.
pub type OverlayState = Arc<RwLock<Option<MatchOverlay>>>;

fn write_atomic(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, content).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Render overlay pieces to separate text files. FFmpeg drawtext with textfile=path:reload=1 for each.
pub fn render_overlay_pieces(camera_name: &str, overlay: Option<&MatchOverlay>) {
    let empty = " ".to_string();
    let (p1name, p1rating, p2name, p2rating, raceto, raceto2, score1, score2) = match overlay {
        Some(o) if o.is_practice => (
            format!("Practice: {}", o.player_one.name),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            format!("Rack #{}", o.player_one.games_won + 1),
            empty.clone(),
            empty.clone(),
            empty.clone(),
        ),
        Some(o) => (
            o.player_one.name.clone(),
            o.player_one.rating.clone().unwrap_or_default(),
            o.player_two.name.clone(),
            o.player_two.rating.clone().unwrap_or_default(),
            "race to".to_string(),
            if o.player_one.race_to == o.player_two.race_to {
                o.player_one.race_to.to_string()
            } else {
                format!("{}-{}", o.player_one.race_to, o.player_two.race_to)
            },
            o.player_one.games_won.to_string(),
            o.player_two.games_won.to_string(),
        ),
        None => (
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
        ),
    };
    for (piece, content) in [
        (OVERLAY_P1_NAME, p1name),
        (OVERLAY_P1_RATING, p1rating),
        (OVERLAY_P2_NAME, p2name),
        (OVERLAY_P2_RATING, p2rating),
        (OVERLAY_RACE_TO, raceto),
        (OVERLAY_RACE_TO2, raceto2),
        (OVERLAY_SCORE1, score1),
        (OVERLAY_SCORE2, score2),
    ] {
        let path = overlay_path_for_camera_piece(camera_name, piece);
        write_atomic(&path, &content);
    }
}

/// Restore overlay from any active match in the database. Call at server startup.
pub fn restore_overlay_from_db(
    db: &Db,
    overlay_state: &OverlayState,
    rtmp_processes: &rtmp::RtmpState,
) {
    let cameras = db.list_cameras().ok().unwrap_or_default();
    for camera in cameras {
        if let Some(ref id) = camera.id {
            let has_active_match = db
                .find_active_pool_match_by_camera_id(id)
                .ok()
                .flatten()
                .is_some();
            if camera.camera_type.is_rtsp() && has_active_match {
                update_overlay(db, overlay_state, id, rtmp_processes, None);
                break;
            }
        }
    }
}

/// Spawn a background task that periodically syncs overlay text with DB.
pub fn spawn_overlay_refresh_task(
    db: Db,
    overlay_state: OverlayState,
    _rtmp_processes: rtmp::RtmpState,
) {
    std::thread::spawn(move || {
        let interval = std::time::Duration::from_secs(2);
        loop {
            std::thread::sleep(interval);
            let cameras = match db.list_cameras() {
                Ok(c) => c,
                Err(_) => continue,
            };
            for camera in cameras {
                if camera.camera_type.is_rtsp() {
                    let overlay = overlay_state.read().ok().and_then(|g| (*g).clone());
                    render_overlay_pieces(&camera.name, overlay.as_ref());
                }
            }
        }
    });
}

/// Update the overlay for the camera. Call when match is created/updated.
pub fn update_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_id: &str,
    _rtmp_processes: &rtmp::RtmpState,
    overlay_from_match: Option<MatchOverlay>,
) {
    let camera = match db.find_camera_by_id(camera_id) {
        Ok(Some(c)) => c,
        Ok(None) => return,
        Err(_) => return,
    };

    if !camera.camera_type.is_rtsp() {
        return;
    }

    let overlay = overlay_from_match.or_else(|| {
        db.find_active_pool_match_by_camera_id(camera_id)
            .ok()
            .flatten()
            .filter(|m| m.end_time.is_none())
            .map(|m| MatchOverlay {
                player_one: OverlayPlayer::from_match_player(&m.player_one),
                player_two: OverlayPlayer::from_match_player(&m.player_two),
                is_practice: m.match_type == MatchType::Practice,
            })
    });

    if let Ok(mut guard) = overlay_state.write() {
        *guard = overlay.clone();
    }
    render_overlay_pieces(&camera.name, overlay.as_ref());
}

/// Clear the overlay (e.g. when match ends).
pub fn clear_overlay(
    db: &Db,
    overlay_state: &OverlayState,
    camera_id: &str,
    _rtmp_processes: &rtmp::RtmpState,
) {
    let camera = match db.find_camera_by_id(camera_id) {
        Ok(Some(c)) if c.camera_type.is_rtsp() => c,
        _ => return,
    };
    let current = overlay_state.read().ok().and_then(|g| (*g).clone());
    if current.is_none() {
        return;
    }
    if let Ok(mut guard) = overlay_state.write() {
        *guard = None;
    }
    render_overlay_pieces(&camera.name, None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool_match::{MatchPlayer, Rating};

    #[test]
    fn overlay_path_for_camera_piece_sanitizes_name() {
        let path = overlay_path_for_camera_piece("Camera 1", "p1name");
        assert!(path.to_string_lossy().ends_with("rtmp-overlay-Camera_1-p1name.txt"));
    }

    #[test]
    fn overlay_path_for_camera_piece_allows_alphanumeric_dash_underscore() {
        let path = overlay_path_for_camera_piece("cam-01_abc", "score1");
        assert!(path.to_string_lossy().ends_with("rtmp-overlay-cam-01_abc-score1.txt"));
    }

    #[test]
    fn overlay_path_for_camera_piece_empty_default() {
        let path = overlay_path_for_camera_piece("", "p2name");
        assert!(path.to_string_lossy().ends_with("rtmp-overlay-default-p2name.txt"));
    }

    #[test]
    fn overlay_path_for_camera_piece_special_chars_become_underscore() {
        let path = overlay_path_for_camera_piece("cam@home!", "raceto");
        assert!(path.to_string_lossy().ends_with("rtmp-overlay-cam_home_-raceto.txt"));
    }

    #[test]
    fn overlay_path_in_data_dir() {
        let path = overlay_path_for_camera_piece("test", "p1rating");
        assert!(path.to_string_lossy().contains("data"));
    }

    #[test]
    fn overlay_player_from_match_player_apa_rating() {
        let p = MatchPlayer {
            name: "Alice".to_string(),
            race_to: 9,
            games_won: 3,
            rating: Some(Rating::Apa(5)),
        };
        let overlay = OverlayPlayer::from_match_player(&p);
        assert_eq!(overlay.name, "Alice");
        assert_eq!(overlay.rating.as_deref(), Some("APA 5"));
        assert_eq!(overlay.games_won, 3);
        assert_eq!(overlay.race_to, 9);
    }

    #[test]
    fn overlay_player_from_match_player_fargo_rating() {
        let p = MatchPlayer {
            name: "Bob".to_string(),
            race_to: 7,
            games_won: 2,
            rating: Some(Rating::Fargo(650)),
        };
        let overlay = OverlayPlayer::from_match_player(&p);
        assert_eq!(overlay.rating.as_deref(), Some("Fargo 650"));
    }

    #[test]
    fn overlay_player_from_match_player_no_rating() {
        let p = MatchPlayer {
            name: "Charlie".to_string(),
            race_to: 5,
            games_won: 0,
            rating: None,
        };
        let overlay = OverlayPlayer::from_match_player(&p);
        assert_eq!(overlay.rating, None);
    }
}
