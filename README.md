# Encore

A desktop player for YouTube Music, written in native Rust with [iced](https://iced.rs).

The interface takes its cues from Cider: a sidebar with the library, a home page with a background
image and a grid of recent plays, a built-in player at the bottom, and a synced lyrics panel on the
right.

## Status

Phases 1 to 7 are done: search and playback (downloaded with yt-dlp, played with mpv), signing
in with your browser's YouTube Music session, the library and playlists, offline downloads, synced
lyrics and the immersive view, media keys (MPRIS), Discord Rich Presence, Last.fm scrobbling, and
the audio extras: seek, volume, shuffle and repeat, a 10-band equalizer with presets, volume
normalization, playback speed and crossfade. Phase 7 added themes (built-in, custom JSON files and
an accent color taken from the cover) and Pear-style plugins: desktop notifications and
SponsorBlock, plus switches for Discord and Last.fm. Settings are under the gear in the top bar.

The queue button in the top bar swaps the lyrics panel for the queue: the song that is playing and
the ones up next, which you can jump to, move up to play next or take out. The + on any song row
adds it to the end of the queue (or plays it when nothing is queued).

Songs follow each other with no gap: once the next song is downloaded, it is queued in mpv, which
goes on into it without reopening the sound output. With crossfade on, the fade takes over.

### Listen together

The people button in the top bar opens the together screen. **Host** opens a port (41730 by
default) and makes an invite (`encore:<address>:<port>:<token>`); friends paste it under **Join**,
or, with the Discord plugin on, use "Ask to Join" on your Discord profile or an invite you send
from a chat (their Encore must be open). Each Encore downloads the songs itself and follows the
host's song, position and pauses. Friends on the internet need your router to forward the port.
They connect straight to your computer, so they see its address and you see theirs. Details:
[`docs/superpowers/specs/2026-11-14-encore-together-design.md`](docs/superpowers/specs/2026-11-14-encore-together-design.md).

Runtime requirements: `mpv` (libmpv) and `yt-dlp` installed on the system.

### Last.fm

Scrobbling needs a Last.fm API account (create one at
<https://www.last.fm/api/account/create>; the callback URL can stay empty). Encore reads it from
two environment variables and says on the account screen which one is missing:

```sh
ENCORE_LASTFM_API_KEY=...
ENCORE_LASTFM_API_SECRET=...
```

Then press "Connect Last.fm" on the account screen and approve Encore in the browser. The session
key and the scrobbles waiting to be sent are kept in `~/.local/share/encore/lastfm/`, readable only
by you.

- Design: [`docs/superpowers/specs/2026-11-12-encore-fase1-design.md`](docs/superpowers/specs/2026-11-12-encore-fase1-design.md)
- Step plan: [`docs/superpowers/plans/2026-11-12-encore-fase1.md`](docs/superpowers/plans/2026-11-12-encore-fase1.md)

## Planned stack

- Rust (2024 edition)
- iced for the interface
- rustypipe for the YouTube Music catalog, yt-dlp for audio downloads, mpv (libmpv) for playback
