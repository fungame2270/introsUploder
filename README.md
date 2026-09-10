# Jellyfin Intro Uploader

A vibecoded solution to upload and manage intro videos for your Jellyfin media server.

## Features

- Drag & drop video uploads
- Video preview and playback
- Video management (delete)
- Resync button to detect files already in the intro folder
- Up to 4GB file support
- Docker support

## Quick Start

### Docker (Recommended)

```bash
docker compose up -d
```

### Manual

```bash
cargo build --release
VIDEO_DIR=./videos PORT=3000 ./target/release/video-manager
```

Then open http://localhost:3000 in your browser.

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `VIDEO_DIR` | `./videos` | Directory to store intro videos |
| `PORT` | `3000` | Port to run the server on |

## Jellyfin Setup

Set the `VIDEO_DIR` to your Jellyfin intros folder (e.g., `/var/lib/jellyfin/media/intros`) and mount it as a volume in Docker.

## Resync Button

If you add videos directly to the folder (e.g., copying files manually), click the **Resync** button to scan the folder and update the metadata. This will:

1. Detect new video files not tracked by the app
2. Remove entries for deleted files
3. Rebuild the metadata database
