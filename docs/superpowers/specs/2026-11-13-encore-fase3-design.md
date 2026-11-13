# Encore — phase 3: account, library and downloads

Date: 2026-11-13
Status: built (2026-11-13)

## Signing in

- YouTube Music's library endpoints (history, saved songs, albums, artists, playlists, liked
  music) go through the web client, which rustypipe authenticates **only with session cookies**.
  rustypipe's OAuth device-code login works only for the TV client, so it cannot load a library.
- "Sign in with <default browser>" opens music.youtube.com with `xdg-open`; the user signs in with
  Google there; Encore polls every 5 s (up to 3 minutes) by asking yt-dlp to read the browser's
  cookies (`--cookies-from-browser`). Zen is passed as `firefox:<profile>`; its profiles are looked
  up in `~/.config/zen`, `~/.zen` and the Flatpak equivalents. Other browsers: Firefox, Chromium,
  Chrome, Brave. Plan B: import a `cookies.txt`.
- yt-dlp dumps every browser cookie into a temporary file; Encore keeps only `youtube.com` cookies
  and deletes the file at once. rustypipe stores the session in
  `~/.local/share/encore/rustypipe/` (mode 700) and Encore restores it at startup.
- The library needs rustypipe's `userdata` feature.

## Library

- Sidebar: History, Songs, Albums, Artists, Liked music, Downloads, and the user's playlists
  (replacing the sample ones; the duplicate "Liked Music" playlist, id `LM`, is hidden).
- Only the first page of each list is loaded for now.

## Downloads

- "Download" on any album, playlist or artist page saves every song, one at a time, into
  `~/.local/share/encore/offline/<video id>.<ext>` (copied from the cache when already there).
- `index.json` in that folder lists the downloaded collections with their songs. No database.
- Playback always prefers the offline copy, so downloaded music plays without a connection.
- "Downloaded · Remove" deletes the collection and the songs no other download uses.
- Not yet: offline covers (images still come from the network) and paging past the first page.
