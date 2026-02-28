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

static INTERNAL_CAMERA: OnceLock<Arc<InternalCameraState>> = OnceLock::new();

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
    let scale = (h as f32 / 280.0).max(10.0);
    let line_h = (scale * 1.1) as i32;
    let bar_h = (line_h * 2 + 20).min(h - 4).max(32);
    let bar_y = (h - bar_h).max(0);
    let y = bar_y + 6;

    // Solid dark background bar
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
                tracing::error!("Failed to open internal camera: {}", e);
                return;
            }
        };

        if let Err(e) = camera.open_stream() {
            tracing::error!("Failed to start camera stream: {}", e);
            return;
        }

        tracing::info!("Internal camera stream started");

        loop {
            match camera.frame() {
                Ok(buffer) => {
                    let jpeg_bytes = if buffer.source_frame_format() == FrameFormat::MJPEG {
                        let raw = buffer.buffer();
                        let overlay_guard = overlay_state.read().ok();
                        let overlay = overlay_guard.as_ref().and_then(|g| g.as_ref());

                        if let (Some(overlay), Some(font)) = (overlay, font.as_ref()) {
                            if let Ok(dyn_img) = load_from_memory(raw) {
                                let mut rgb = dyn_img.to_rgb8();
                                draw_overlay(&mut rgb, overlay, font);
                                let mut jpeg = Vec::new();
                                if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
                                    .encode_image(&rgb)
                                    .is_ok()
                                {
                                    Bytes::from(jpeg)
                                } else {
                                    Bytes::copy_from_slice(raw)
                                }
                            } else {
                                Bytes::copy_from_slice(raw)
                            }
                        } else {
                            Bytes::copy_from_slice(raw)
                        }
                    } else {
                        match buffer.decode_image::<RgbFormat>() {
                            Ok(mut rgb) => {
                                let overlay_guard = overlay_state.read().ok();
                                let overlay = overlay_guard.as_ref().and_then(|g| g.as_ref());
                                if let (Some(overlay), Some(font)) = (overlay, font.as_ref()) {
                                    draw_overlay(&mut rgb, overlay, font);
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
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Frame capture error: {}", e);
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(33));
        }

        let _ = camera.stop_stream();
        tracing::info!("Internal camera stream stopped");
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
        *guard = overlay;
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
        let (tx, _) = broadcast::channel(2);
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

/// POST /api/cameras/:id/stream/rtmp - Start RTMP push to the given URL.
/// Spawns FFmpeg to read the MJPEG stream and push to RTMP (e.g. YouTube Live, Facebook).
/// Requires FFmpeg to be installed. The overlay is burned into the stream.
pub async fn camera_stream_rtmp_start(
    State(app): State<crate::api::AppState>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<RtmpStartRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = app
        .db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        return Err(ApiError::BadRequest(
            "RTMP export only available for internal cameras".to_string(),
        ));
    }

    if req.url.is_empty() || !req.url.starts_with("rtmp://") {
        return Err(ApiError::BadRequest(
            "url must be a valid RTMP URL (e.g. rtmp://...)".to_string(),
        ));
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let stream_url = format!("http://127.0.0.1:{}/api/cameras/{}/stream", port, id);

    let ffmpeg = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            &stream_url,
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-tune",
            "zerolatency",
            "-f",
            "flv",
            &req.url,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match ffmpeg {
        Ok(_) => Ok(axum::Json(serde_json::json!({ "ok": true, "message": "RTMP stream started" }))),
        Err(e) => Err(ApiError::BadRequest(format!(
            "Failed to start FFmpeg: {}. Ensure FFmpeg is installed.",
            e
        ))),
    }
}

#[derive(serde::Deserialize)]
pub struct RtmpStartRequest {
    pub url: String,
}
