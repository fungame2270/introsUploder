use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

use crate::error::AppError;
use crate::metadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEntry {
    pub id: String,
    pub original_filename: String,
    pub stored_filename: String,
    pub size: u64,
    pub duration: Option<f64>,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AppState {
    pub video_dir: PathBuf,
}

fn metadata_path(state: &AppState) -> PathBuf {
    state.video_dir.join("videos.json")
}

async fn load_metadata(state: &AppState) -> Result<Vec<VideoEntry>, AppError> {
    let path = metadata_path(state);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    let entries: Vec<VideoEntry> = serde_json::from_str(&data)?;
    Ok(entries)
}

async fn save_metadata(state: &AppState, entries: &[VideoEntry]) -> Result<(), AppError> {
    let path = metadata_path(state);
    let data = serde_json::to_string_pretty(entries)?;
    fs::write(path, data)?;
    Ok(())
}

pub async fn list_videos(
    State(state): State<AppState>,
) -> Result<Json<Vec<VideoEntry>>, AppError> {
    let entries = load_metadata(&state).await?;
    Ok(Json(entries))
}

pub async fn upload_video(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<VideoEntry>, AppError> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            original_filename = field.file_name().map(|s| s.to_string());
            let data = field.bytes().await?;
            file_data = Some(data.to_vec());
        }
    }

    let data = file_data.ok_or_else(|| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No file field found in multipart upload",
        ))
    })?;
    let orig_name = original_filename.unwrap_or_else(|| "untitled.mp4".to_string());

    let id = Uuid::new_v4().to_string();
    let ext = std::path::Path::new(&orig_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    let stored_name = format!("{id}.{ext}");

    let file_path = state.video_dir.join(&stored_name);
    fs::write(&file_path, &data)?;

    let duration = metadata::get_duration_secs(&file_path);
    let entry = VideoEntry {
        id,
        original_filename: orig_name,
        stored_filename: stored_name,
        size: data.len() as u64,
        duration,
        uploaded_at: Utc::now(),
    };

    let mut entries = load_metadata(&state).await?;
    entries.push(entry.clone());
    save_metadata(&state, &entries).await?;

    Ok(Json(entry))
}

fn parse_range(header: &str, file_size: u64) -> Option<(u64, u64)> {
    let range_str = header.strip_prefix("bytes=")?;
    let (start_str, end_str) = range_str.split_once('-')?;
    let start: u64 = start_str.parse().ok()?;
    let end: u64 = if end_str.is_empty() {
        file_size - 1
    } else {
        end_str.parse().ok()?
    };
    if start <= end && end < file_size {
        Some((start, end))
    } else {
        None
    }
}

pub async fn stream_video(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let entries = load_metadata(&state).await?;
    let entry = entries.iter().find(|e| e.id == id).ok_or_else(|| {
        AppError::NotFound(format!("Video {id} not found"))
    })?;

    let file_path = state.video_dir.join(&entry.stored_filename);
    let file_size = fs::metadata(&file_path)?.len();

    let mime = mime_guess::from_path(&entry.stored_filename)
        .first_or_octet_stream()
        .to_string();

    let range_header = headers.get(RANGE).and_then(|v| v.to_str().ok());

    if let Some(range_str) = range_header {
        if let Some((start, end)) = parse_range(range_str, file_size) {
            let content_length = end - start + 1;
            let mut file = tokio::fs::File::open(&file_path).await?;
            file.seek(std::io::SeekFrom::Start(start)).await?;

            let mut buf = vec![0u8; content_length as usize];
            file.read_exact(&mut buf).await?;

            let response = Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(CONTENT_TYPE, &mime)
                .header(CONTENT_LENGTH, content_length)
                .header(CONTENT_RANGE, format!("bytes {start}-{end}/{file_size}"))
                .header("accept-ranges", "bytes")
                .body(Body::from(buf))
                .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            return Ok(response);
        }
    }

    let buf = fs::read(&file_path)?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, &mime)
        .header(CONTENT_LENGTH, file_size)
        .header("accept-ranges", "bytes")
        .body(Body::from(buf))
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(response)
}

pub async fn delete_video(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut entries = load_metadata(&state).await?;
    let index = entries.iter().position(|e| e.id == id).ok_or_else(|| {
        AppError::NotFound(format!("Video {id} not found"))
    })?;

    let entry = entries.remove(index);
    let file_path = state.video_dir.join(&entry.stored_filename);
    if file_path.exists() {
        fs::remove_file(&file_path)?;
    }

    save_metadata(&state, &entries).await?;
    Ok(StatusCode::NO_CONTENT)
}
