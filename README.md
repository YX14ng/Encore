# Encore

A desktop player for YouTube Music, written in native Rust with [iced](https://iced.rs).

The interface takes its cues from Cider: a sidebar with the library, a home page with a background
image and a grid of recent plays, a built-in player at the bottom, and a synced lyrics panel on the
right.

## Status

Phases 1 to 7 are done: search and playback (downloaded with yt-dlp, played with mpv), signing
in to YouTube Music, the library and playlists, offline downloads, synced
lyrics and the immersive view, media keys (MPRIS), Discord Rich Presence, Last.fm scrobbling, and
the audio extras: seek, volume, shuffle and repeat, a 10-band equalizer with presets, volume
normalization, playback speed and crossfade. Phase 7 added themes (built-in, custom JSON files and
an accent color taken from the cover) and Pear-style plugins: desktop notifications and
SponsorBlock, plus switches for Discord and Last.fm. Settings are under the gear in the top bar.

Signing in happens in a small window of Encore's own (WebKitGTK), the way Pear Desktop does it:
you sign in to Google there, and the session belongs to Encore alone. A session read from your
browser stops working as soon as that browser renews its cookies, often within minutes, so reading
it from a browser or a cookies.txt is only offered as another way. The window keeps nothing on
disk; Encore keeps only the YouTube cookies, in `~/.local/share/encore/rustypipe/`.

The queue button in the top bar swaps the lyrics panel for the queue: the song that is playing and
the ones up next, which you can jump to, move up to play next or take out. The + on any song row
adds it to the end of the queue (or plays it when nothing is queued).

With autoplay on (the switch in the queue panel, on by default), the queue does not end: when
the last song starts, Encore asks YouTube Music for that song's radio and adds the songs it does not
already hold under "Autoplay", where they play after the ones you picked. Songs you add with + go
before them. Clearing the queue brings new similar songs; turning autoplay off (or repeat) makes
the queue stop at its end. A guest in a listen-together session follows the host instead.

Signed in, "New playlist" in the sidebar makes a playlist on your YouTube Music account (private,
unlisted or public). The playlist button on any song row adds it to one of your playlists, or to a
new one; in your own playlists the ✕ takes a song out. Any playlist can have a cover of your own
(the Cover button on its page): YouTube Music has no way to change it, so the picture is kept on
this computer, as a 600 px square in `~/.local/share/encore/covers/`, and shows in the sidebar, the
cards and the playlist page.

An artist opens on a page of its own: the banner with the name and subscribers, Play (the top
songs), the artist's radio and Download, then the top songs (See all opens every song), albums,
singles and EPs, playlists that feature the artist, similar artists and what YouTube Music says
about them. Explore, in the sidebar, shows moods, new releases, the charts (YouTube Music gives
them as playlists for your country), top artists, new music videos and genres; a mood or genre
opens a page of playlists. Back goes to the screen you came from.

Signed in, "Radio & mixes" shows the mixes YouTube Music makes for you (My Supermix, Discover
Mix, the mood mixes…) and radios from your history; "Artists" lists the artists of the songs in
your library, "Subscriptions" the ones you subscribed to, and "Podcasts" your saved podcasts and
"Episodes for later". rustypipe has no call for these pages, so Encore reads their cards from
YouTube Music's answer (`src/browse.rs`).

Songs follow each other with no gap: once the next song is downloaded, it is queued in mpv, which
goes on into it without reopening the sound output. With crossfade on, the fade takes over.

The audio screen (the equalizer button in the player bar) also picks the sound output (the
system's default or any PipeWire device; a saved device that is unplugged gives way to the
default), has a compressor that evens out loud and quiet parts (Light, Medium or Strong, between
the equalizer and normalization), and can skip silence: ffmpeg's `silencedetect` listens to the
first 20 and last 30 seconds of each downloaded song, and mpv starts and ends the song where its
music does (`src/silence.rs`). Silences inside a song stay.

The audio quality is Low (Opus around 60 kbit/s), Normal (around 130 kbit/s, the best YouTube
gives anyone) or High: with YouTube Music Premium, Opus around 256 kbit/s. For High, yt-dlp gets
the signed-in session in a cookies file that only the user can read
(`~/.local/share/encore/yt-dlp/`), written for that one download and deleted when yt-dlp ends.
yt-dlp warns that accounts used this way can get blocked for a while, so High is off by default.
Each quality has its own cache folder (`~/.cache/encore/audio-low`, `audio`, `audio-high`).

The thumbs in the player bar like and dislike the playing song on YouTube Music (signed in;
`src/rating.rs`). YouTube does not say how a song was rated, so a song shows as liked when it is
in Liked Music and as disliked when it was disliked in Encore, which keeps those in
`~/.local/share/encore/disliked.json`. With the "Skip disliked songs" plugin (on by default),
disliking the playing song skips it, and so does a disliked song that comes up again.

Lyrics come from YouTube Music alone (`src/lyrics_source.rs`): the synced lyrics its phone app
shows, which follow the song line by line, or the plain lyrics of its Lyrics tab when a song has
no synced ones. There are no translations.

The mini player button in the player bar shrinks the window to the cover, the song and the
controls and keeps it above other windows; the same button brings the full window back. The badge
next to the song shows the playing file's codec and average bitrate (YouTube's Opus is usually
around 130 kbit/s).

### Listen together

The people button in the top bar opens the together screen. **Host** opens a port (41730 by
default) and makes an invite (`encore:<address>:<port>:<token>`); friends paste it under **Join**,
or, with the Discord plugin on, use "Ask to Join" on your Discord profile or an invite you send
from a chat (their Encore must be open). Each Encore downloads the songs itself and follows the
host's song, position and pauses. Friends on the internet need your router to forward the port.
They connect straight to your computer, so they see its address and you see theirs. Details:
[`docs/superpowers/specs/2026-11-14-encore-together-design.md`](docs/superpowers/specs/2026-11-14-encore-together-design.md).

Runtime requirements: `mpv` (libmpv) and `yt-dlp` installed on the system; `ffmpeg` for skipping
silence (without it songs play whole).

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
- WebKitGTK (webkit2gtk 4.1) for the sign-in window
- rustypipe for the YouTube Music catalog, yt-dlp for audio downloads, mpv (libmpv) for playback
