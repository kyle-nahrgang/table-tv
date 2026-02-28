# Table TV

A simple app with an API and UI, served together.

## Quick Start (Docker)

1. Build and run:

   ```bash
   docker compose up --build
   ```

2. Open in your browser:
   - **<http://localhost>** or **<http://127.0.0.1>**
   - For **<http://table-tv.local>**, add to `/etc/hosts`: `127.0.0.1 table-tv.local`

## Local Development

**Terminal 1 – API** (auto-reloads on changes; requires [cargo-watch](https://crates.io/crates/cargo-watch): `cargo install cargo-watch`):

```bash
cd api && cargo watch -x run
```

**Terminal 2 – UI:**

```bash
cd ui && npm run dev
```

The UI proxies `/api` to the API. Open <http://localhost:5173>.

To reset the database (e.g. if `initialized` is wrong): delete `api/data/` and restart the API.

## RTMP streaming (Go Live)

RTMP export (YouTube, Facebook, etc.) uses **GStreamer** via the Rust bindings. The API requires GStreamer to build and run.

- **macOS:** Install [GStreamer binaries](https://gstreamer.freedesktop.org/download/) (recommended over Homebrew for development). Or: `brew install gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly` and set `PKG_CONFIG_PATH` to the lib/pkgconfig directory.
- **Ubuntu/Debian:** `sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly`
