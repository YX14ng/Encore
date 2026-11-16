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
disk; Encore keeps only the YouTube cookies, in `~/.local/share/encore/rustypipe/`. While Encore
is open it renews a session from its window the way an open YouTube tab does, about every ten
minutes; a session read from a browser is left alone, since renewing it would sign the browser
out. When YouTube stops accepting the session, a banner offers to sign in again and the saved
library stays on screen; the account screen says how long the session has worked.

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
The two songs after it are downloaded ahead too. Signed in, a song that is not in the cache yet
takes about three seconds (yt-dlp starting and the download); signed out, YouTube puts an ad
before it and its stream only works once the ad would have played, so yt-dlp waits about six
seconds more. Meanwhile the player bar already shows it with "Loading…" and the previous song
stops.

Shuffle is Encore's own, not YouTube's: albums, playlists and the library's song lists have a
Shuffle button, and Play with shuffle on starts from a random song too, so no song always comes
first. The order is a uniform random one (Fisher-Yates, seeded anew each start); songs of a
playlist that load after it started playing land at random places among those still to play.
The library loads every saved and liked song, so the oldest ones are in the mix too.

Long lists (a playlist of a thousand songs, the library's songs, likes and history) only build
the rows in sight and download covers as they scroll into view (`src/virtual_list.rs`). A long
playlist shows its first hundred songs at once and loads the rest a page at a time; albums and
playlists opened lately show at once as they were left while a fresh copy loads.

Synced lyrics can be scrolled by hand (they slide back to the line being sung four seconds
later), and clicking a line jumps to where it starts.

Japanese, Korean and Chinese lyrics also show each line in the Latin alphabet, small under it
(the "Romanized lyrics" plugin, on by default; songs in the interface's language are left alone).
Japanese is split into words with lindera and its IPADIC dictionary, which reads kanji in context
(君 is "kimi"), and written in Hepburn ("kimi no namae wo yonde iru"); Korean follows the Revised
Romanization with its sound changes ("byeolbichi", "gamsahamnida"); Chinese is pinyin with tone
marks. The dictionary is downloaded once while building and embedded in the binary, which makes
it about 45 MB larger (`src/romanize/`).

The audio screen (the equalizer button in the player bar) also picks the sound output (the
system's default or any PipeWire device; a saved device that is unplugged gives way to the
default), has a compressor that evens out loud and quiet parts (Light, Medium or Strong, between
the equalizer and normalization), and can skip silence: ffmpeg's `silencedetect` listens to the
first 20 and last 30 seconds of each downloaded song, and mpv starts and ends the song where its
music does (`src/silence.rs`). Silences inside a song stay.

The mouse wheel over the volume bar moves the volume by 1, 2, 5 or 10 % a notch (picked on the
audio screen; touchpad scrolls add up to notches), and the bar shows the percentage while the
pointer is over it.

The audio quality is Low (Opus around 60 kbit/s), Normal (around 130 kbit/s, the best YouTube
gives anyone) or High: with YouTube Music Premium, Opus around 256 kbit/s. Signed in, yt-dlp
gets the session for every download (with Premium that means no ad and no wait for one), in a
cookies file that only the user can read (`~/.local/share/encore/yt-dlp/`), written for that one
download and deleted when yt-dlp ends. yt-dlp warns that accounts used this way can get blocked
for a while; the audio screen says so.
Each quality has its own cache folder (`~/.cache/encore/audio-low`, `audio`, `audio-high`).

The thumbs in the player bar like and dislike the playing song on YouTube Music (signed in;
`src/rating.rs`). YouTube does not say how a song was rated, so a song shows as liked when it is
in Liked Music and as disliked when it was disliked in Encore, which keeps those in
`~/.local/share/encore/disliked.json`. With the "Skip disliked songs" plugin (on by default),
disliking the playing song skips it, and so does a disliked song that comes up again.

Lyrics come from YouTube Music alone (`src/lyrics_source.rs`): the synced lyrics its phone app
shows, which follow the song line by line, or the plain lyrics of its Lyrics tab when a song has
no synced ones. There are no translations.

Clicking the song in the player bar opens the now playing page, like Pear's
(`src/now_playing.rs`): it slides up over the window with the cover (or the music video) in the
middle, a Song / Video switch above it, and the queue and the lyrics in tabs beside it; the
chevron or Escape closes it. The video button opens it straight on the video, and in the full
screen view the video fills the screen instead (`src/video/`). Songs on YouTube Music are album audio; signed in, YouTube Music names their music
video and where the two line up, and a music video that plays as itself is its own picture.
yt-dlp downloads the picture alone (up to 720p, into `~/.cache/encore/video`) and ffmpeg turns it
into frames that follow the song's position; the sound still comes from mpv, with the equalizer
and the rest. Closing the page (or leaving full screen) turns the video off.

The Visualizer plugin (on by default) shows bars that move with the music in the full screen
view (`src/visualizer.rs`): when a song starts, ffmpeg decodes its file and an FFT measures 48
bands, 30 times a second, which the bars read at mpv's position.

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

The interface comes in 15 languages (`src/i18n.rs`, texts in `locales/*.yml`): English,
简体中文, 繁體中文, हिन्दी, Español, العربية, Français, বাংলা, Português (Brasil), Русский, Bahasa
Indonesia, Deutsch, Italiano, 日本語 and 한국어 — the ten most spoken languages, Traditional
Chinese, three of the largest music markets, and Italian. It follows the system's language unless another is picked
under Settings → Appearance, and YouTube Music is asked for its own texts in the same language.
Arabic is translated, but the layout is not mirrored right to left.

Runtime requirements: `mpv` (libmpv) and `yt-dlp` installed on the system; `ffmpeg` for skipping
silence, the visualizer and music videos (without it songs play whole, with no bars or video).

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

## License

Encore is free software: you can redistribute it and/or modify it under the terms of the GNU
General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. It is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY; see `LICENSE` for the full text. The fonts, the Japanese
dictionary and the libraries it uses keep their own licenses; see `THIRD_PARTY.md`.
