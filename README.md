# Encore

A desktop player for YouTube Music, written in native Rust with [iced](https://iced.rs).

The interface takes its cues from Cider: a sidebar with the library, a home page with a background
image and a grid of recent plays, a built-in player at the bottom, and a synced lyrics panel on the
right.

## Status

Phase 2 is done: search YouTube Music from the sidebar and play songs (downloaded with yt-dlp,
played with mpv). Next is phase 3: signing in, the library and playlists.

Runtime requirements: `mpv` (libmpv) and `yt-dlp` installed on the system.

- Design: [`docs/superpowers/specs/2026-11-12-encore-fase1-design.md`](docs/superpowers/specs/2026-11-12-encore-fase1-design.md)
- Step plan: [`docs/superpowers/plans/2026-11-12-encore-fase1.md`](docs/superpowers/plans/2026-11-12-encore-fase1.md)

## Planned stack

- Rust (2024 edition)
- iced for the interface
- rustypipe for the YouTube Music catalog, yt-dlp for audio downloads, mpv (libmpv) for playback
