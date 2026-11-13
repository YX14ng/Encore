# Encore — phase 2: search and play

Date: 2026-11-13
Status: built (2026-11-13)

## Goal

Search YouTube Music from the sidebar and play a result with real audio.

## Decisions

- **Catalog:** `rustypipe` 0.11 (`music_search_tracks`). Search works; its last release is from
  April 2025.
- **Audio download:** `yt-dlp` (system package). rustypipe can list the audio streams, but its
  own download fails ("could not extract sig fn name", outdated deobfuscation), and its stream URLs
  answer 403 to mpv. yt-dlp solves YouTube's JS challenges (with deno) and downloads the file.
- **Playback:** mpv through `libmpv2` 7, playing the downloaded file. Streaming a URL straight into
  mpv returns 403, so Encore always downloads first ("download, then play").
- **Cache:** `$XDG_CACHE_HOME/encore/audio/<video id>.<ext>` (Opus in WebM when available). A cached
  file plays instantly; while a song plays, the next result is downloaded in the background.
- **Downloaded playlists (later):** the same files, kept in a permanent folder instead of the cache,
  plus a small index. No large database (Pear's 8 GB IndexedDB is the cautionary tale).
- **License:** rustypipe is GPL-3.0. Fine while Encore is private; publishing Encore means GPL-3.0.

## What works

- Typing in the sidebar search box and pressing Enter shows "Results for …" with covers, artist,
  album and duration.
- Clicking a result shows "Loading…", downloads it (about 7 seconds the first time, YouTube makes
  yt-dlp wait) and plays it: the player bar, the cover and the blurred home background switch to it.
- Play/pause, the progress bar and "next" drive mpv; at the end of a song the next result starts.
- Real songs show "No lyrics for this track" until phase 4.
- The sample data still fills the home screen and plays as before until a real song starts.

## Verification

- 47 unit tests, plus `cargo test -- --ignored searches_downloads_and_plays`, which searches,
  downloads and decodes audio through mpv with the sound output off (passed in 11.7 s).
- The results screen was rendered offscreen with `iced_test` to check the layout.
