# Encore — phase 4: lyrics and the immersive view

Date: 2026-11-13
Status: built (2026-11-13)

## Lyrics

- Source order for the song that starts: disk cache (`~/.cache/encore/lyrics/<video id>.json`),
  then LRCLIB (`/api/get` with title, first artist, album and length; then `/api/search`, keeping
  the result within 4 s of the song's length and preferring synced lyrics), then the plain lyrics
  YouTube Music shows (rustypipe `music_details` → `music_lyrics`).
- Title variants tried on LRCLIB: as is, without bracketed parts like "(feat. …)", and each side
  of "Original - Romanized" titles such as "うっせぇわ - Usseewa".
- "Not found" is not cached, so a later try can find lyrics added to LRCLIB.
- LRC times are read to the millisecond. While a song plays, the panel follows mpv's position on
  every animation frame (about 30 per second), not only on the one-second clock.
- The panel shows synced lines (animated), plain lines (scrollable), "Instrumental", "Looking
  for lyrics…" or "No lyrics for this track".

## Immersive view

- The full-screen button of the player bar (or Escape to leave) switches the window to full
  screen and shows the blurred cover filling the window, a 380 px cover with title and artist,
  large lyrics beside it, and the player bar at the bottom.
