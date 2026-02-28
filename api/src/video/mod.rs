//! Internal camera MJPEG streaming.

use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use nokhwa::{
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use polodb_core::bson::oid::ObjectId;
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::db::Db;
use crate::error::ApiError;

const MJPEG_BOUNDARY: &str = "frame";

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

/// Spawn the camera capture task. Runs on a blocking thread and broadcasts
/// JPEG frames to all subscribers.
fn spawn_camera_capture(tx: broadcast::Sender<Bytes>) {
    std::thread::spawn(move || {
        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
            RequestedFormatType::AbsoluteHighestFrameRate,
        );

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
                    let jpeg_bytes = if buffer.source_frame_format()
                        == nokhwa::utils::FrameFormat::MJPEG
                    {
                        // Raw buffer is already JPEG
                        Bytes::copy_from_slice(buffer.buffer())
                    } else {
                        // Decode to RGB and encode to JPEG
                        match buffer.decode_image::<nokhwa::pixel_format::RgbFormat>() {
                            Ok(rgb) => {
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

/// GET /api/cameras/:id/stream - MJPEG stream for internal cameras.
pub async fn camera_stream(
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let oid = ObjectId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid camera id".to_string()))?;

    let camera = db
        .find_camera_by_id(&oid)?
        .ok_or(ApiError::CameraNotFound)?;

    if !camera.camera_type.is_internal() {
        return Err(ApiError::BadRequest(
            "Stream only available for internal cameras".to_string(),
        ));
    }

    let state = INTERNAL_CAMERA.get_or_init(|| {
        let (tx, _) = broadcast::channel(2);
        spawn_camera_capture(tx.clone());
        Arc::new(InternalCameraState::new(tx))
    });

    let rx = state.subscribe();
    let stream = BroadcastStream::new(rx)
    .map(|x| match x {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, e.to_string())),
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
            (header::CONTENT_TYPE, format!("multipart/x-mixed-replace; boundary={}", MJPEG_BOUNDARY)),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate".to_string()),
            (header::PRAGMA, "no-cache".to_string()),
            (header::EXPIRES, "0".to_string()),
        ],
        body,
    )
        .into_response();

    Ok(response)
}
