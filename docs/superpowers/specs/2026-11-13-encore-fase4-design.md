# Encore — phase 4: lyrics and the immersive view

Date: 2026-11-13
Status: built (2026-11-13)

## Lyrics

- Lyrics come from YouTube Music only (changed 2026-11-15; LRCLIB was dropped, and so were
  translations). Source order for the song that starts: disk cache
  (`~/.cache/encore/lyrics/<video id>.ytm.json`), then the synced lyrics YouTube Music's phone app
  shows (the `browse` request for the song's "MPLYt…" lyrics id, sent as the `ANDROID_MUSIC`
  client, which answers a `timedLyricsModel` with each line's start), then the plain lyrics of the
  Lyrics tab (rustypipe `music_details` → `music_lyrics`).
- "Not found" is not cached, so a later try (after a failed request) can find them.
- LRC times are read to the millisecond. While a song plays, the panel follows mpv's position on
  every animation frame (about 30 per second), not only on the one-second clock.
- The panel shows synced lines (animated), plain lines (scrollable), "Instrumental", "Looking
  for lyrics…" or "No lyrics for this track".

## Immersive view

- The full-screen button of the player bar (or Escape to leave) switches the window to full
  screen and shows the blurred cover filling the window, a 380 px cover with title and artist,
  large lyrics beside it, and the player bar at the bottom.
