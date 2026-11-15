# Encore: overall design and phase 1

Date: 2026-11-12
Status: approved (2026-11-12); phase 1 closed (2026-11-13), see section 8

## 1. Intent

Encore is a desktop player for YouTube Music, written in native Rust with iced. It has two goals,
and both matter equally:

1. **Replace Pear Desktop** (the Electron app the user runs today) with a native app that looks
   like Cider (the alternative Apple Music client) and, in the long run, does everything Cider and
   Pear do, but for YouTube Music.
2. **Learn Rust from scratch.** The user has never written Rust or iced. They have read the course
   "Rust语言圣经" (https://beatai.org/rust-course) up to the chapter 所有权和借用 (ownership and
   borrowing). Encore is the project they learn with.

Because of the second goal, **the user types all the code**. The assistant explains each concept,
says where each piece of code goes and what to write, reviews the result and commits it. The
assistant never edits the app's source files.

## 2. Product goals (all phases)

Reference list built from Cider (cider.sh) and the Pear plugins (`pear-devs/pear-desktop`, folder
`src/plugins`):

- **Interface (from Cider):** home page with recommendations and recent plays, album and artist
  pages, synced lyrics in a side panel that follow the song line by line (with translation and
  romanization), immersive mode, mini player, themes and customization. No karaoke mode: the
  lyrics only move along with the song.
- **Audio:** 10-band equalizer, playback speed, crossfade, compressor, silence skipping, precise
  and exponential volume, output device selection.
- **Integrations:** system media controls (MPRIS on Linux), notifications, Discord Rich Presence,
  Last.fm scrobbling and remote control through a local API.
- **YouTube-specific (from Pear):** SponsorBlock, skipping disliked tracks, switching between video
  and audio, quality selection, downloads, turning off autoplay, colors taken from the cover art, a
  visualizer and group listening.
- **Extensibility:** a plugin system.

### Phase order (agreed)

1. **Visual skeleton:** the main screen in iced with fake data. *(this document)*
2. **Play a track:** search YouTube Music and play the result.
3. **Your account:** login, library and playlists.
4. **Lyrics:** the side panel with real data, and immersive mode.
5. **Integrations:** MPRIS, Discord and Last.fm.
6. **Audio extras:** equalizer, crossfade and the rest.
7. **Themes and plugins.**

Each phase gets its own design document when its turn comes. The planned data sources for phase 2
onward (not part of phase 1) are: the YouTube Music internal API (InnerTube), probably through the
`rustypipe` crate with `yt-dlp` as a fallback for streams; an audio engine (mpv or GStreamer, to be
decided in phase 2); and LRCLIB for synced lyrics.

## 3. Non-goals and anti-goals

**Non-goals:**
- AirPlay, Apple Music Sing and Cider's own spatial audio (they do not carry over to YouTube
  Music).
- Wrapping the YouTube Music website in a webview, as Pear and Tauri apps do. Encore is native.
- Windows and macOS: not blocked on purpose, but not tested either. *(assistant's call)*

**Anti-goals** (things that make the project fail even if the app works):
- The user typing code they do not understand. The user must understand what they type, so every
  step comes with an explanation.
- The assistant editing the app's source files.
- Using "YouTube" in the app's name or branding.
- An interface that does not look like the approved mockup.

## 4. Constraints

- **User's platform:** Arch Linux (CachyOS), the niri compositor on Wayland, Rust 1.99 installed
  with pacman (no rustup).
- **Repo:** `~/Projects/Encore`, private remote `YX14ng/Encore`, branch `main`.
- **Commits:** Conventional Commits 1.0.0 style, in English: `<type>: <summary>`, with the types
  `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf` and `ci`. The summary is imperative
  and lowercase. One type of change per commit. Never commit while verification is failing.
- **Small files:** 200 to 400 lines is typical, 800 is the limit. Files are organized by feature,
  not by type.
- **Language:** identifiers, UI text, commit messages and the repo docs are in English.
- **Code style (user's decision, 2026-11-13):** the
  [Google Rust Style Guide](https://chromium.googlesource.com/external/github.com/google/styleguide/+/HEAD/rustguide.md).
  In practice: rustfmt at 100 columns; one `use` per module and no glob imports except
  `use super::*` in tests; doc comments on every `pub` item that do not restate the name;
  `Option` for obvious failures, `Result` when the caller needs context, `expect` with a reason
  instead of `unwrap` (enforced by `clippy::unwrap_used = "deny"`); named fields over tuples;
  explicit types on `.parse()` and `.into()` when the target is not obvious; traits imported
  inside the function that uses them; `Default` next to every argument-less `new()`. Its
  Chromium- and Fuchsia-only rules (gtest, `chromium::import!`, no async) do not apply, and the
  options that need nightly rustfmt (`hex_literal_case`, `imports_granularity`) are followed by
  hand.

## 5. Working mode (agreed with the user)

> **Changed on 2026-11-13 (user's decision):** short on time, the user asked for speed. From the
> end of phase 1 on, the assistant writes and applies the changes directly and summarizes them per
> phase. The step-by-step tutoring below describes how phase 1 was built.

Each step of the guide has five parts, in this order:

1. **Explanation:** which Rust or iced concept shows up in this step and why it is needed, with a
   small example that is not Encore's code.
2. **Reading:** the exact chapter of the Rust语言圣经 course that covers that concept, and nothing
   more.
3. **Objective and code:** what the step must achieve. From step 3 on, the assistant also says
   where each piece of code goes and what to write or change: the exact file, the location and the
   code. Steps 1 and 2 used hints only.
4. **The user types and runs** the code with `cargo run`.
5. **Review:** the assistant reviews the code and what shows on screen, and explains any compiler
   errors and their cause. Once the step is right and verification passes, the assistant commits
   it.

Steps start very small and grow as the user gains confidence. A step introduces at most one major
new concept.

**Straight into iced (user's decision).** There are no warm-up exercises in plain Rust: the guide
goes straight to the window. An empty iced window already uses structs, enums, `impl`, `match`,
closures, external crates and, later, generics and lifetimes (`Element<'_, Message>`). Code the user
cannot understand yet is marked with a `// step N` comment, meaning "use it now, step N explains
it". Step N exists in the plan and explains it. Each step's explanation lists which pieces are of
this kind. No piece is left without a step that explains it.

**The step plan** lives in the repo (`docs/superpowers/plans/`). It stays at the level of
objective, explanation summary, reading, hints, tests and verification. The exact code for each
step is given in the chat when the step starts. *(user's decision)*

**Non-code files** (SVG icons, the Inter font, the sample covers and the sample text: titles,
artists, playlists and lyrics) come from the assistant, because typing them teaches no Rust. The
user writes the structures and the code that use them. *(assistant's call)*

## 6. Phase 1: visual skeleton

### 6.1 Objective

Encore's main window in iced, matching the approved mockup
(https://claude.ai/artifact/XesHz5mLWNanJGaGLLvSUP), with fake data written in the code. No
network, no YouTube and no audio.

### 6.2 What it shows

A window with three columns:

- **Sidebar (left, about 236 px):** a search box (decorative in this phase) and three collapsible
  sections:
  - *YouTube Music:* Home, Explore, Radio & mixes.
  - *Library:* History, Songs, Albums, Artists, Subscriptions, Podcasts.
  - *Playlists:* New playlist, Liked music, and five sample playlists ("Study", "J-Pop", "Rainy
    night", "Gym" and "My Supermix"), each with a small colored square.
  Each entry has an icon in the accent color. The selected entry is highlighted.
- **Center:**
  - *Top bar:* back, forward and show/hide sidebar on the left; listen together, cast, lyrics
    (active) and queue on the right. Only the sidebar and lyrics buttons do anything in this phase.
  - *Home (scrollable):* a large background image with the greeting on top ("Good morning", "Good
    afternoon" or "Good evening", depending on local time) and a fade to dark at the bottom. Below
    it, "Recently played", a 3-column grid with 6 sample rows (cover, title, artist and a `···`
    button), and "Mixed for you", a horizontal row of 5 square cards.
  - *Player (fixed at the bottom):* a progress bar with elapsed and remaining time; shuffle,
    previous, play/pause, next, repeat and like; a card with the cover, title, artist and audio
    format ("OPUS 160"); volume, mini player and fullscreen.
- **Lyrics panel (right, about 300 px):** timed sample lyrics. The current line is white and
  larger; the others are dimmed. The list scrolls by itself to keep the current line near the top
  third, with a fade at the top and bottom.

**Home background:** the cover of the playing track, blurred (the user's final decision, after
trying "chosen image" and coming back to this option). Encore loads the cover, shrinks it, applies
a strong blur with the `image` crate once per track, and draws the result larger than the home
area, with a fade to dark at the bottom so the greeting stays readable. A slow pan or zoom gives it
movement. If the iced version makes that hard, a still image is fine for a first version.

**Sample covers:** three or four small square images stored in `assets/covers/`, supplied by the
assistant. They are used for the rows, the mixes, the player card and as the source of the blurred
background. To see the background change with the track, the "next" button moves to the next
sample track (the only exception to "next does nothing" in section 6.3).

**Visual reference:** the mockup is the reference for appearance, but where this document says
otherwise, the document wins. Phase 1 leaves out the mockup's background picker, custom window
buttons and any narrow-window adaptations not listed in section 6.3. The top bar's `···` button is
drawn with no function. The mockup's lyrics style (past lines dimmer, a slight blur and an enlarged
current line) is optional. *(assistant's call)*

**Look:** dark theme only, near black, with a pinkish-red accent. The font is Inter (bundled with
the app), with the system font as a fallback. The colors come from the mockup's tokens:

| Token | Value | Use |
|---|---|---|
| page background | `#0a0a0c` | behind everything |
| sidebar | `#131315` | |
| center | `#19191c` | |
| lyrics panel | `#151517` | |
| text | `#f3f3f5` | |
| secondary text | `#9b9ba4` | |
| dim text | `#5f5f68` | |
| accent | `#ff4459` | icons, like, `···` |
| lines | white at 7 % | borders and separators |

### 6.3 What responds

- **Play/pause:** starts and stops a fake clock that advances one second per second. The progress
  bar, the times and the current lyrics line all follow that clock. When it reaches the end, the
  fake track starts over.
- **Like:** toggles the thumb's color.
- **Show or hide** the sidebar and the lyrics panel from the top bar.
- **Collapse and expand** each sidebar section.
- **Selecting a sidebar entry** highlights it. It does not change screens yet.
- **Narrow window:** below about 1100 px wide the lyrics panel hides; below about 720 px, the
  sidebar hides too.

- **Next:** moves to the next sample track (changes the cover, background and title, and restarts
  the clock).

Everything else (search, back and forward, previous, queue, cast, volume, mini player and
fullscreen) is drawn but does nothing.

### 6.4 Code structure

Organized by feature, with small files. This is a guide; exact names get adjusted during the steps:

```
src/
  main.rs          iced startup and window configuration
  app.rs           global state, enum Message, update, view and subscription (the clock)
  theme.rs         colors, fonts and shared styles
  mock.rs          fake data: current track, recents, mixes, playlists and lyrics
  sidebar.rs       sidebar
  home.rs          home: blurred background, greeting, recents and mixes
  player.rs        bottom player
  lyrics.rs        lyrics panel and current line calculation
assets/
  fonts/           Inter
  icons/           SVG icons
  covers/          sample covers (also the source of the blurred background)
```

Pure logic lives apart from the view so it can be tested without opening windows: formatting `93`
as `1:33`, computing the remaining time, picking the lyrics line for a given second, and the
greeting for a given hour.

### 6.5 Verification

- `scripts/check.sh` runs `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`. The
  user writes it in one of the first steps, and it runs before every commit.
- Unit tests for the pure logic in section 6.4.
- The visual part is checked by running the app and comparing it with the mockup. The assistant
  does not open windows on the user's PC (a bot runs on another screen): the user runs it and sends
  a screenshot.

### 6.6 Definition of done

- The window looks like the mockup in all three columns, with the colors from the table.
- Everything in section 6.3 works.
- `scripts/check.sh` passes.
- The user can explain in their own words what `Message`, `update`, `view` and the clock
  subscription do.

## 7. Left to whoever writes the plan

- The exact iced version: the latest stable one when work starts, with the features it needs
  (images, SVG, tokio for the clock).
- How to animate the lyrics scrolling with what that iced version offers (a jump with no animation
  is fine for a first version).
- Window decorations: phase 1 uses the system ones. Drawing custom minimize, maximize and close
  buttons like Cider is optional. *(assistant's call)*
- The order and size of the guide's steps, following section 5 and starting with: hello world with
  `cargo new`, an empty iced window, a Play/Pause button, the three columns as colored blocks, and
  then filling them in.

## 8. Phase 1 outcome (2026-11-13)

Everything in section 6.3 works, `scripts/check.sh` passes with 42 tests, and the window matches the
mockup. Decisions made along the way:

- **Code style:** the Google Rust Style Guide (section 4), applied in a dedicated refactor step.
- **Lyrics:** every line has a fixed row height, the panel scrolls to the current line with a
  500 ms ease-out animation and fades at the top and bottom. Dimmed lines use the system monospace
  font in semibold (user's taste); the current line uses Inter bold.
- **Background:** the blurred cover zooms slowly between 1.02x and 1.14x over 30 seconds while a
  track plays. Animation frames run at about 30 per second, only while something moves.
- **Typography:** Inter (Regular, SemiBold, Bold) is bundled in `assets/fonts/` with its OFL license.
- **Verification without opening windows:** `iced_test` can render the whole view offscreen to a
  PNG, which is how the animations were checked. It is not part of the repo yet.
- **Skipped:** the definition-of-done item where the user explains `Message`, `update`, `view` and
  the subscription in their own words, because of the change of working mode.
