//! Internal camera MJPEG streaming with match overlay.

use ab_glyph::FontRef;
use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use image::{load_from_memory, Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_circle_mut, draw_text_mut};
use imageproc::rect::Rect;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType},
    Camera,
};
use polodb_core::bson::oid::ObjectId;
use std::sync::{Arc, RwLock, OnceLock};
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::db::pool_match::{MatchPlayer, Rating};
use crate::db::Db;
use crate::error::ApiError;

const MJPEG_BOUNDARY: &str = "frame";

/// Overlay data for an active match. Displayed at bottom of stream.
#[derive(Clone, Debug)]
pub struct MatchOverlay {
    pub player_one: OverlayPlayer,
    pub player_two: OverlayPlayer,
}

#[derive(Clone, Debug)]
pub struct OverlayPlayer {
    pub name: String,
    pub rating: Option<String>,
    pub games_won: u8,
    pub race_to: u8,
}

impl OverlayPlayer {
    fn from_match_player(p: &MatchPlayer) -> Self {
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

/// Active RTMP streams: camera_id -> stop sender. Send to stop the stream.
pub type RtmpState = Arc<RwLock<std::collections::HashMap<String, std::sync::mpsc::Sender<()>>>>;

pub fn rtmp_state_new() -> RtmpState {
    Arc::new(RwLock::new(std::collections::HashMap::new()))
}

static INTERNAL_CAMERA: OnceLock<Arc<InternalCameraState>> = OnceLock::new();

/// Pre-initialize the internal camera capture loop at startup. Ensures the stream is ready
/// before any requests (e.g. when user starts match first, then goes live via OAuth).
pub fn ensure_internal_camera_ready(overlay: OverlayState) {
    let _ = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        spawn_camera_capture(tx.clone(), overlay);
        Arc::new(InternalCameraState::new(tx))
    });
}

/// Restore overlay from any active match in the database. Call at server startup.
pub fn restore_overlay_from_db(db: &Db, overlay_state: &OverlayState) {
    if let Ok(Some(internal)) = db.find_internal_camera() {
        update_overlay(db, overlay_state, &internal.name);
    }
}

/// Shared state for the internal camera stream.
pub struct InternalCameraState {
    tx: broadcast::Sender<Bytes>,
}

impl InternalCameraState {
    fn new(tx: broadcast::Sender<Bytes>) -> Self {
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.tx.subscribe()
    }
}

fn load_font() -> Option<FontRef<'static>> {
    let font_bytes = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
    FontRef::try_from_slice(font_bytes).ok()
}

/// Draw the match overlay onto an RGB image. Bottom bar with player names, ratings, score.
fn draw_overlay(img: &mut RgbImage, overlay: &MatchOverlay, font: &FontRef) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    if w < 1 || h < 1 {
        return;
    }
    let scale = (h as f32 / 280.0).max(10.0);
    let line_h = (scale * 1.1) as i32;
    let bar_h = (line_h * 2 + 20).min(h - 4).max(32).min(h);
    let bar_y = (h - bar_h).max(0);
    let y = bar_y + 6;

    // Solid dark background bar (clamp to image bounds)
    let bar_rect = Rect::at(0, bar_y).of_size(w as u32, bar_h as u32);
    draw_filled_rect_mut(img, bar_rect, Rgb([0u8, 0, 0]));

    let white = Rgb([255u8, 255, 255]);
    let gray = Rgb([180u8, 180, 180]);
    let scale_sm = scale * 0.65;

    // Player 1 (left): name, then rating below if present
    draw_text_mut(
        img,
        white,
        12,
        y,
        ab_glyph::PxScale::from(scale),
        font,
        &overlay.player_one.name,
    );
    if let Some(ref r) = overlay.player_one.rating {
        draw_text_mut(img, gray, 12, y + line_h, ab_glyph::PxScale::from(scale_sm), font, r);
    }

    // Center: [circle] score1 | race to \n X/Y | [circle] score2
    let score_scale = scale * 1.1;
    let s1 = overlay.player_one.games_won.to_string();
    let s2 = overlay.player_two.games_won.to_string();
    let race_line1 = "race to";
    let race_line2 = format!("{}/{}", overlay.player_one.race_to, overlay.player_two.race_to);

    let (s1_w, s1_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s1);
    let (race1_w, race1_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, race_line1);
    let (race2_w, _) = imageproc::drawing::text_size(ab_glyph::PxScale::from(scale_sm), font, &race_line2);
    let (s2_w, s2_h) = imageproc::drawing::text_size(ab_glyph::PxScale::from(score_scale), font, &s2);

    let race_w = race1_w.max(race2_w);
    let gap = 12;
    let total_w = s1_w as i32 + gap + race_w as i32 + gap + s2_w as i32;
    let cx = (w - total_w) / 2;
    let s1_x = cx;
    let race_x = cx + s1_w as i32 + gap;
    let s2_x = cx + s1_w as i32 + gap + race_w as i32 + gap;

    let circle_r = (s1_h.max(s2_h) as i32 / 2) + 4;
    let s1_cy = y + s1_h as i32 / 2;
    let s2_cy = y + s2_h as i32 / 2;

    // Draw circles around both scores
    draw_hollow_circle_mut(img, (s1_x + s1_w as i32 / 2, s1_cy), circle_r, white);
    draw_hollow_circle_mut(img, (s2_x + s2_w as i32 / 2, s2_cy), circle_r, white);

    draw_text_mut(img, white, s1_x, y, ab_glyph::PxScale::from(score_scale), font, &s1);
    draw_text_mut(img, gray, race_x, y, ab_glyph::PxScale::from(scale_sm), font, race_line1);
    draw_text_mut(img, gray, race_x, y + race1_h as i32 + 2, ab_glyph::PxScale::from(scale_sm), font, &race_line2);
    draw_text_mut(img, white, s2_x, y, ab_glyph::PxScale::from(score_scale), font, &s2);

    // Player 2 (right): name, then rating below if present
    let p2_w_approx = (overlay.player_two.name.len() as f32 * scale * 0.6) as i32;
    let p2_x = (w - p2_w_approx - 16).max(cx + total_w + 24);
    draw_text_mut(
        img,
        white,
        p2_x,
        y,
        ab_glyph::PxScale::from(scale),
        font,
        &overlay.player_two.name,
    );
    if let Some(ref r) = overlay.player_two.rating {
        draw_text_mut(img, gray, p2_x, y + line_h, ab_glyph::PxScale::from(scale_sm), font, r);
    }
}

/// Spawn the camera capture task. Composites overlay onto frames when match is active.
fn spawn_camera_capture(tx: broadcast::Sender<Bytes>, overlay_state: OverlayState) {
    std::thread::spawn(move || {
        let font = load_font();
        if font.is_none() {
            tracing::warn!("Failed to load font, overlay will be skipped");
        }

        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let mut camera = match Camera::new(index, requested) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Internal camera not available (expected in Docker): {}", e);
                return;
            }
        };

        if let Err(e) = camera.open_stream() {
            tracing::warn!("Failed to start camera stream: {}", e);
            return;
        }

        tracing::info!("Internal camera stream started");

        loop {
            match camera.frame() {
                Ok(buffer) => {
                    // Clone overlay quickly and release lock before expensive draw/encode.
                    let overlay = overlay_state
                        .read()
                        .ok()
                        .and_then(|g| g.clone());

                    let jpeg_bytes = if buffer.source_frame_format() == FrameFormat::MJPEG {
                        let raw = buffer.buffer();
                        if let (Some(overlay), Some(font)) = (overlay.as_ref(), font.as_ref()) {
                            let overlay_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                load_from_memory(raw).ok().and_then(|dyn_img| {
                                    let mut rgb = dyn_img.to_rgb8();
                                    draw_overlay(&mut rgb, overlay, font);
                                    let mut jpeg = Vec::new();
                                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
                                        .encode_image(&rgb)
                                        .ok()
                                        .map(|_| Bytes::from(jpeg))
                                })
                            }));
                            match overlay_result {
                                Ok(Some(bytes)) => bytes,
                                Ok(None) | Err(_) => {
                                    if overlay_result.is_err() {
                                        tracing::warn!("Overlay panic, using raw frame");
                                    } else {
                                        tracing::debug!("Overlay apply failed (load/encode), using raw frame");
                                    }
                                    Bytes::copy_from_slice(raw)
                                }
                            }
                        } else {
                            Bytes::copy_from_slice(raw)
                        }
                    } else {
                        match buffer.decode_image::<RgbFormat>() {
                            Ok(mut rgb) => {
                                if let (Some(overlay), Some(font)) = (overlay.as_ref(), font.as_ref()) {
                                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        draw_overlay(&mut rgb, overlay, font);
                                    }))
                                    .is_err()
                                    {
                                        tracing::warn!("Overlay panic, using frame without overlay");
                                    }
                                }
                                let mut jpeg = Vec::new();
                                if let Err(e) = image::codecs::jpeg::JpegEncoder::new_with_quality(
                                    &mut jpeg,
                                    80,
                                )
                                .encode_image(&rgb)
                                {
                                    tracing::warn!("JPEG encode error: {}", e);
                                    continue;
                                }
                                Bytes::from(jpeg)
                            }
                            Err(e) => {
                                tracing::warn!("Frame decode error: {}", e);
                                continue;
                            }
                        }
                    };

                    if tx.send(jpeg_bytes).is_err() {
                        // No subscribers (e.g. browser + FFmpeg both disconnected).
                        // Keep running so camera stays ready for new connections.
                        tracing::debug!("No stream subscribers, continuing capture");
                    }
                }
                Err(e) => {
                    tracing::warn!("Frame capture error: {}", e);
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(33));
        }
    });
}

/// Update the overlay for the internal camera. Call when match is created/updated.
/// Sets overlay to None when no active match.
pub fn update_overlay(db: &Db, overlay_state: &OverlayState, camera_name: &str) {
    let internal = match db.find_internal_camera() {
        Ok(Some(c)) => c,
        _ => return,
    };
    if internal.name != camera_name {
        return;
    }

    let overlay = match db.find_active_pool_match_by_camera_name(camera_name) {
        Ok(Some(m)) if m.end_time.is_none() => Some(MatchOverlay {
            player_one: OverlayPlayer::from_match_player(&m.player_one),
            player_two: OverlayPlayer::from_match_player(&m.player_two),
        }),
        _ => None,
    };

    if let Ok(mut guard) = overlay_state.write() {
        let is_set = overlay.is_some();
        *guard = overlay;
        if is_set {
            tracing::info!(camera = %camera_name, "Overlay set for active match");
        }
    }
}

/// Clear the overlay (e.g. when match ends).
pub fn clear_overlay(db: &Db, overlay_state: &OverlayState, camera_name: &str) {
    let internal = match db.find_internal_camera() {
        Ok(Some(c)) => c,
        _ => return,
    };
    if internal.name != camera_name {
        return;
    }
    if let Ok(mut guard) = overlay_state.write() {
        *guard = None;
    }
}

/// GET /api/cameras/:id/stream - MJPEG stream for internal cameras.
pub async fn camera_stream(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = app
        .db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        return Err(ApiError::BadRequest(
            "Stream only available for internal cameras".to_string(),
        ));
    }

    let state = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        spawn_camera_capture(tx.clone(), app.overlay.clone());
        Arc::new(InternalCameraState::new(tx))
    });

    let rx = state.subscribe();
    let stream = BroadcastStream::new(rx)
        .map(|x| match x {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                e.to_string(),
            )),
        })
        .map(|bytes: Result<Bytes, _>| {
            bytes.map(|bytes| {
                let header = format!(
                    "\r\n--{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    MJPEG_BOUNDARY,
                    bytes.len()
                );
                let out: Bytes = [header.as_bytes(), bytes.as_ref()].concat().into();
                out
            })
        });

    let body = Body::from_stream(stream);

    let response = (
        [
            (
                header::CONTENT_TYPE,
                format!("multipart/x-mixed-replace; boundary={}", MJPEG_BOUNDARY),
            ),
            (
                header::CACHE_CONTROL,
                "no-cache, no-store, must-revalidate".to_string(),
            ),
            (header::PRAGMA, "no-cache".to_string()),
            (header::EXPIRES, "0".to_string()),
        ],
        body,
    )
        .into_response();

    Ok(response)
}

/// Spawn a GStreamer pipeline that reads MJPEG from stream_url and pushes to rtmp_url.
/// Runs in a thread; stops when stop_rx receives. Removes from rtmp_processes when done.
fn spawn_rtmp_pipeline(
    stream_url: &str,
    rtmp_url: &str,
    stop_rx: std::sync::mpsc::Receiver<()>,
    rtmp: RtmpState,
    id: String,
) -> Result<(), String> {
    use gstreamer::prelude::*;

    let pipeline_desc = format!(
        "audiotestsrc wave=4 ! audioconvert ! audioresample ! voaacenc bitrate=128000 ! \
         aacparse ! queue ! mux. \
         souphttpsrc location=\"{}\" do-timestamp=true ! multipartdemux ! jpegdec ! \
         videoconvert ! x264enc tune=zerolatency speed-preset=1 key-int-max=60 ! \
         h264parse ! queue ! mux. \
         flvmux streamable=true name=mux ! rtmpsink location=\"{}\"",
        stream_url.replace('"', "\\\""),
        rtmp_url.replace('"', "\\\"")
    );

    let pipeline = gstreamer::parse::launch(&pipeline_desc)
        .map_err(|e| format!("Pipeline parse error: {}", e))?;

    let pipeline = pipeline
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| "Expected Pipeline element")?;

    let bus = pipeline.bus().expect("Pipeline has bus");

    pipeline
        .set_state(gstreamer::State::Playing)
        .map_err(|e| format!("Failed to set pipeline to Playing: {}", e))?;

    std::thread::spawn(move || {
        let _pipeline = pipeline;
        loop {
            match stop_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }

            if let Some(msg) = bus.timed_pop(gstreamer::ClockTime::from_mseconds(100)) {
                use gstreamer::MessageView;
                match msg.view() {
                    MessageView::Eos(..) => break,
                    MessageView::Error(err) => {
                        tracing::error!(
                            camera_id = %id,
                            "GStreamer error: {} (debug: {:?})",
                            err.error(),
                            err.debug()
                        );
                        break;
                    }
                    MessageView::Warning(warn) => {
                        tracing::warn!(
                            camera_id = %id,
                            "GStreamer warning: {}",
                            warn.error()
                        );
                    }
                    _ => {}
                }
            }
        }

        let _ = _pipeline.set_state(gstreamer::State::Null);
        rtmp.write().unwrap().remove(&id);
        tracing::info!(camera_id = %id, "RTMP: GStreamer pipeline ended");
    });

    Ok(())
}

/// POST /api/cameras/:id/stream/rtmp - Start RTMP push to the given URL.
/// Spawns GStreamer to read the MJPEG stream and push to RTMP (e.g. YouTube Live, Facebook).
/// Requires GStreamer (gst-plugins-good, gst-plugins-bad, gst-plugins-ugly) to be installed.
/// The overlay is burned into the stream.
pub async fn camera_stream_rtmp_start(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<RtmpStartRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let url_safe = if req.url.len() > 60 {
        format!("{}...", &req.url[..60])
    } else {
        req.url.clone()
    };
    tracing::info!(camera_id = %id, url = %url_safe, "RTMP start: received request");
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = app
        .db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        tracing::warn!(camera_id = %id, "RTMP start: camera is not internal");
        return Err(ApiError::BadRequest(
            "RTMP export only available for internal cameras".to_string(),
        ));
    }

    if req.url.is_empty()
        || (!req.url.starts_with("rtmp://") && !req.url.starts_with("rtmps://"))
    {
        tracing::warn!(url = %req.url, "RTMP start: invalid URL");
        return Err(ApiError::BadRequest(
            "url must be a valid RTMP URL (e.g. rtmp://... or rtmps://...)".to_string(),
        ));
    }

    // Stop any existing stream for this camera before starting a new one.
    if let Some(stop_tx) = app.rtmp_processes.write().unwrap().remove(&id) {
        tracing::info!("RTMP start: stopping existing stream first");
        let _ = stop_tx.send(());
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, id);
    tracing::info!(stream_url = %stream_url, "RTMP start: starting GStreamer pipeline");

    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let rtmp = app.rtmp_processes.clone();
    let id_clone = id.clone();
    let rtmp_url = req.url.clone();

    match spawn_rtmp_pipeline(&stream_url, &rtmp_url, stop_rx, rtmp.clone(), id_clone) {
        Ok(()) => {
            rtmp.write().unwrap().insert(id, stop_tx);
            tracing::info!("RTMP start: GStreamer pipeline started successfully");
            Ok(axum::Json(serde_json::json!({ "ok": true, "message": "RTMP stream started" })))
        }
        Err(e) => {
            tracing::error!(error = %e, "RTMP start: failed to start GStreamer pipeline");
            Err(ApiError::BadRequest(format!(
                "Failed to start GStreamer pipeline: {}. Ensure GStreamer and plugins are installed (gst-plugins-good, gst-plugins-bad, gst-plugins-ugly).",
                e
            )))
        }
    }
}

#[derive(serde::Deserialize)]
pub struct RtmpStartRequest {
    pub url: String,
}

/// POST /api/cameras/:id/stream/rtmp/stop - Stop the RTMP stream for this camera.
pub async fn camera_stream_rtmp_stop(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let stop_tx = app
        .rtmp_processes
        .write()
        .unwrap()
        .remove(&id)
        .ok_or_else(|| ApiError::BadRequest("No active RTMP stream for this camera.".to_string()))?;

    if stop_tx.send(()).is_err() {
        tracing::warn!(camera_id = %id, "RTMP stop: pipeline thread already ended");
    }

    tracing::info!(camera_id = %id, "RTMP stop: stream stopped");
    Ok(axum::Json(serde_json::json!({ "ok": true, "message": "RTMP stream stopped" })))
}

/// GET /api/cameras/:id/stream/rtmp/status - Check if RTMP stream is active.
pub async fn camera_stream_rtmp_status(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let active = app.rtmp_processes.read().unwrap().contains_key(&id);
    Ok(axum::Json(serde_json::json!({ "active": active })))
}
