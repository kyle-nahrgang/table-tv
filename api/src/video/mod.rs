//! Video streaming: camera sources, overlays, and RTMP export.

use bytes::Bytes;
use tokio::sync::broadcast;

mod mediamtx;
mod mjpeg;
pub(crate) mod overlay;
pub(crate) mod rtsp_camera;
pub(crate) mod rtmp;

pub use overlay::{
    clear_overlay, overlay_path_for_camera, overlay_path_for_camera_piece,
    resolve_overlay_paths_for_camera, restore_overlay_from_db, spawn_overlay_refresh_task,
    update_overlay, MatchOverlay, OverlayPaths, OverlayPlayer, OverlayState,
};
pub use mediamtx::{
    delete_camera_path, fetch_camera_connection_status, finish_recording_segment, is_available,
    mediamtx_rtsp_url, sync_all_paths, sync_camera_path,
};
pub use rtmp::{rtmp_state_new, RtmpStartRequest, RtmpState};

/// Trait for camera sources that provide a video stream.
/// Implementations grab frames and broadcast them to subscribers.
pub trait CameraSource: Send + Sync {
    /// Subscribe to receive MJPEG frame bytes from the stream.
    fn subscribe(&self) -> broadcast::Receiver<Bytes>;
}
