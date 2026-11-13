# Encore phase 1: step guide

> **How to use this guide:** the assistant writes it and the user follows it, typing all the code.
> Each step has an **Explanation** (a summary; the assistant gives the full explanation in the chat
> before the step starts), **Reading**, **Objective**, **Hints**, **Verification** and **Commit**.
> This guide does not hold the code. From step 3 on, the assistant gives the exact code in the chat
> at each step: which file, where in it, and what to write or change, always after explaining the
> concepts involved and pointing to the course reading. The user types it and runs it; the
> assistant reviews it and commits. Steps marked **(assistant)** are done by the assistant because
> they are not code (sample files, icons and fonts). Each `- [ ]` box is ticked once the assistant
> has reviewed the step and committed it.

**Objective:** Encore's main window in iced, matching the mockup, with fake data.

**Architecture:** an iced app with the state → `Message` → `update` → `view` pattern, plus a
subscription for the clock. Pure logic (formatting, clock, lyrics, greeting, layout) lives in small
functions with tests; the view is split into one file per screen area.

**Stack:** Rust 1.99 (2024 edition), iced 0.14 with the `tokio`, `image` and `svg` features, the
`image` 0.25 crate for the blur and the `chrono` crate for local time.

**Spec:** `docs/superpowers/specs/2026-11-12-encore-fase1-design.md`

**Readings:** all of them are from the Rust语言圣经 course. The path `basic/compound-type/struct`
stands for `https://beatai.org/rust-course/basic/compound-type/struct`; the Chinese name is the one
in the course index.

## Global constraints

- The user types all the code in `src/` and `scripts/`. The assistant explains, says where and what
  to write, reviews and commits.
- Code that is not understood yet is marked with a `// step N` comment, and step N explains it.
- Identifiers, UI text, commit messages and docs are in English.
- Files of 200 to 400 lines, 800 at most, organized by screen area.
- Conventional Commits in English (`<type>: <summary>`, imperative, lowercase summary), one type per
  commit, and only with `scripts/check.sh` green (from step 2 on).
- Colors: page `#0a0a0c`, sidebar `#131315`, center `#19191c`, lyrics `#151517`, text `#f3f3f5`,
  secondary `#9b9ba4`, dim `#5f5f68`, accent `#ff4459`, lines white at 7 %.
- Widths: sidebar 236 px, lyrics panel 300 px. The lyrics hide below a 1100 px window and the
  sidebar below 720 px.
- The assistant does not open windows on the PC: the user runs the app and sends a screenshot.

## Review focus

Failures the spec does not name but a user would run into. Each one has its test in the step that
introduces it:

1. **Messy lyrics file or odd lines** (`[ar:...]` metadata, empty lines, malformed times): invalid
   lines are ignored, lines are sorted by time, and nothing panics (step 15).
2. **Track without lyrics:** the panel shows "No lyrics for this track" and nothing indexes out of
   range (step 15).
3. **Elapsed time greater than the duration** (when changing tracks or after one tick too many):
   the remaining time does not go negative or overflow; it is `0:00` (steps 5 and 6).
4. **Cover that cannot be decoded:** the background falls back to a solid color instead of closing
   the app (step 12).
5. **Window resizing combined with the hide buttons:** if the window is narrow the panel hides even
   when its button says "show", and it comes back when the window grows (step 16).

---

### Step 1: hello world with Cargo

**Explanation:** what Cargo is, what `Cargo.toml` and `src/main.rs` are for, and the difference
between `cargo build`, `cargo run` and `cargo check`. The repo already exists, so use `cargo init`
instead of `cargo new`.

**Reading:** `first-try/cargo` (认识 Cargo), `first-try/hello-world` (不仅仅是 Hello world).

**Objective:** `cargo run` in `~/Projects/Encore` prints `Encore arrancando`.

**Hints:**
- `cargo init --name encore` inside the repo. Check what it added to `.gitignore` and whether it
  clashes with the existing one.
- Check that `Cargo.toml` says `edition = "2024"`.

**Verification:** `cargo run` prints `Encore arrancando`.

- [x] **Commit:** `chore: initialize the cargo project`

### Step 2: verification script

**Explanation:** what `cargo fmt`, `cargo clippy` and `cargo test` do, and why a script stops at
the first error (`set -euo pipefail`).

**Reading:** none from the course.

**Objective:** `scripts/check.sh` runs, in this order, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test`, and exits with a non-zero code if
anything fails.

**Hints:** first line `#!/usr/bin/env bash`; make it executable with `chmod +x`. Test it by
breaking the formatting on purpose (an extra space) and checking that it fails.

**Verification:** `scripts/check.sh` finishes with no errors on the hello world.

- [x] **Commit:** `chore: add verification script`

### Step 3: empty iced window

**Explanation:** what an external crate is and how to add one; the general idea of iced (the app
is a state, and iced calls `view` to draw it). This step uses pieces you have not read about yet,
all marked: `struct` and `impl` (step 4), `enum Message` (step 4), `#[derive(...)]` (step 10),
`Element<'_, Message>` (step 11).

**Reading:** `basic/base-type/function` (函数, review).

**Objective:** `cargo run` opens a 1280×800 window titled "Encore" with the text "Encore"
centered.

**Hints:**
- `cargo add iced@0.14 --features tokio,image,svg`.
- `main` returns `iced::Result`.
- `iced::application(App::default, App::update, App::view)` with `.title("Encore")`,
  `.window_size((1280.0, 800.0))` and `.run()`.
- `App` is an empty struct with `#[derive(Default)]`; `Message` is an enum with any single variant
  for now.
- Signatures: `fn update(&mut self, message: Message)` and `fn view(&self) -> Element<'_, Message>`.
- To center: `container(text("Encore")).center(Fill)`.

**Verification:** the window opens (send a screenshot) and `scripts/check.sh` passes.

- [x] **Commit:** `feat: open the Encore window`

### Step 4: Play/Pause button

**Explanation:** the iced loop: the user touches something → a `Message` is produced → `update`
changes the state → `view` draws it again. This is where `struct`, `impl`, `enum` and `match` from
step 3 get properly explained.

**Reading:** `basic/compound-type/struct` (结构体), `basic/compound-type/enum` (枚举),
`basic/match-pattern/match-if-let` (match 和 if let), `basic/method` (方法 Method).

**Objective:** a button that says "Play" and switches to "Pause" when pressed, and back again.

**Hints:**
- `App` gets an `is_playing: bool` field; `Message` has a `TogglePlay` variant.
- `button(label).on_press(Message::TogglePlay)`.
- The label comes from an `if` used as an expression.

**Verification:** the button toggles (screenshots of both states); `scripts/check.sh` passes.

- [x] **Commit:** `feat: add play/pause button`

### Step 5: format times, with tests

**Explanation:** modules (`mod`, files, `pub`), unit tests (`#[cfg(test)]`, `#[test]`,
`assert_eq!`) and why subtracting unsigned integers can overflow.

**Reading:** `basic/crate-module/module` (模块 Module), `basic/formatted-output` (格式化输出),
`test/write-tests` (编写测试及控制执行).

**Objective:** two pure functions in `src/clock.rs`:
- `pub fn format_time(secs: u32) -> String`
- `pub fn remaining(elapsed: u32, duration: u32) -> u32`

**Hints:** `format!` with `{:02}` for the seconds; look at `saturating_sub` for the subtraction.

**Tests you need to write** (in the same file):

| Call | Result |
|---|---|
| `format_time(0)` | `"0:00"` |
| `format_time(59)` | `"0:59"` |
| `format_time(93)` | `"1:33"` |
| `format_time(214)` | `"3:34"` |
| `format_time(600)` | `"10:00"` |
| `remaining(93, 214)` | `121` |
| `remaining(214, 214)` | `0` |
| `remaining(300, 214)` | `0` (review focus 3) |

**Verification:** write the tests first and watch them fail (`cargo test`), then write the
functions and watch them pass; `scripts/check.sh` passes.

- [x] **Commit:** `feat: format playback times`

### Step 6: fake clock with a subscription

**Explanation:** what a subscription is (iced sends you messages on its own, like a tick every
second) and what a closure is, which shows up here in `.map(|_| Message::Tick)`.

**Reading:** `advance/functional-programing/closure` (闭包 Closure).

**Objective:** with Play, a `1:33 / -2:01` label advances one second per second; with Pause it
stops. When it reaches the duration, it goes back to `0:00`.

**Hints:**
- In `src/clock.rs`, a `Playback { elapsed: u32, duration: u32, is_playing: bool }` struct with
  `pub fn tick(&mut self)` and `pub fn toggle(&mut self)`. `is_playing` moves here from `App`.
- The subscription: `iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)` while
  playing, `Subscription::none()` otherwise. Register it with `.subscription(App::subscription)`.
- Start with `elapsed: 93, duration: 214` to match the mockup.

**Tests you need to write:**

| Initial state | Action | `elapsed` after |
|---|---|---|
| 93 of 214, playing | `tick()` | 94 |
| 213 of 214, playing | `tick()` | 0 (starts over) |
| 93 of 214, paused | `tick()` | 93 |
| 300 of 214, playing | `tick()` | 0 (review focus 3) |

**Verification:** the counter advances and stops; `scripts/check.sh` passes.

- [x] **Commit:** `feat: simulate the playback clock`

### Step 7: colors and the three columns

**Explanation:** `const`, `use` across modules, and how iced styles a `container` with a closure
that returns a `container::Style`.

**Reading:** `basic/crate-module/use` (使用 use 引入模块及受限可见性).

**Objective:** the window splits into three columns in the spec's colors: a 236 px sidebar, a
center that takes the rest, and a 300 px lyrics panel. The button and the clock stay in the center.

**Hints:**
- `src/theme.rs` with constants such as `pub const SIDEBAR: Color = Color::from_rgb8(0x13, 0x13, 0x15);`,
  one for each color in the spec. The lines: `Color::from_rgba8(255, 255, 255, 0.07)`.
- `row![a, b, c]`; each one is a `container(...).width(236).height(Fill).style(...)`.
- A dark background for the app: `.theme(|_| Theme::Dark)` or a custom `style` on the app.

**Verification:** screenshot with the three columns; `scripts/check.sh` passes.

- [x] **Commit:** `feat: lay out the three columns`

### Step 8: top bar and hiding columns

**Explanation:** `Option` as "there is something or there isn't", and how iced accepts an `Option`
as a child (if it is `None`, nothing is drawn). `bool::then`.

**Reading:** `basic/match-pattern/option` (解构 Option).

**Objective:** the center has a bar at the top with the `‹`, `›`, sidebar, `···`, listen together,
cast, lyrics and queue buttons (as text for now; the icons arrive in step 10). The sidebar and
lyrics buttons hide and show their column.

**Hints:**
- `App` gets `show_sidebar: bool` and `show_lyrics: bool`, both starting as `true`.
- `Message::ToggleSidebar` and `Message::ToggleLyrics`.
- `.push(self.show_sidebar.then(|| sidebar_view()))`.
- Split the view: `src/sidebar.rs`, `src/home.rs`, `src/player.rs` and `src/lyrics.rs`, each with a
  `pub fn view(...) -> Element<'_, Message>` function.

**Verification:** both buttons hide and show their column (screenshot); `scripts/check.sh` passes.

- [x] **Commit:** `feat: add top bar with collapsible columns`

### Step 9 (assistant): sample files

The assistant adds:
- `assets/mock/tracks.tsv`: the sample tracks, one per line, tab-separated: title, artist, album,
  duration in seconds, cover file and lyrics file (`-` if there are none, so editors that trim trailing whitespace cannot break the line).
- `assets/mock/recents.tsv` and `assets/mock/mixes.tsv`: title, subtitle and cover.
- `assets/mock/playlists.tsv`: name and hex color.
- `assets/mock/luz-de-neon.lrc`: made-up lyrics in LRC format (`[mm:ss.cc] text`), with one
  `[ar:...]` metadata tag, one empty line and one line out of order on purpose (review focus 1).
- `assets/covers/*.png`: five generated 300×300 covers.

- [x] **Commit (assistant):** `chore: add sample data and covers`

### Step 10: sample data and icons in the sidebar

**Explanation:** `String` vs `&str`, `Vec`, iterators (`lines`, `split`, `filter_map`) and
`include_str!` to embed a file in the executable. Also `#[derive(Debug, Clone, Copy, PartialEq)]`,
left over from step 3.

**Reading:** `basic/compound-type/string-slice` (字符串与切片), `basic/collections/vector`
(动态数组 Vector), `advance/functional-programing/iterator` (迭代器 Iterator), `basic/trait/trait`
(特征 Trait, only the `derive` part).

**Objective:**
- `src/mock.rs` with `pub struct Track { title, artist, album, duration, cover, lyrics }` and
  `pub struct Card { title, subtitle, cover }` (all text as `String`), plus
  `pub fn parse_tracks(text: &str) -> Vec<Track>` and `pub fn parse_cards(text: &str) -> Vec<Card>`.
- The full sidebar: search box (decorative), three collapsible sections, and the playlists from
  `playlists.tsv` with their small colored square. Selecting an entry highlights it.

**Hints:**
- A line without all the fields is skipped (`filter_map` + `Option`).
- `enum Section { Music, Library, Playlists }` and `enum NavItem { Home, Explore, ... }`; the state
  stores which sections are open and which entry is selected.
- `Message::ToggleSection(Section)` and `Message::Select(NavItem)`.
- The assistant supplies the icons in `assets/icons/` along with this step. Load them with
  `svg(svg::Handle::from_memory(include_bytes!("../assets/icons/home.svg")))` and paint them in the
  accent color through their `style`.

**Tests you need to write:**

| `parse_tracks` input | Result |
|---|---|
| a line with all 6 fields | a `Track` with those values and `duration` as a number |
| a line with 5 fields | skipped |
| a line with duration `abc` | skipped |
| empty lines at the end | ignored |
| the real `tracks.tsv` | 5 tracks |

**Verification:** screenshot of the sidebar with one section collapsed; `scripts/check.sh` passes.

- [x] **Commit:** `feat: build the sidebar from sample data`

### Step 11: home with greeting, recents and mixes

**Explanation:** lifetimes, starting from `Element<'_, Message>` in step 3: the view borrows the
state, and the `'_` says that what gets drawn cannot outlive that borrow. Also `chunks` to build
rows of 3.

**Reading:** `basic/lifetime` (认识生命周期).

**Objective:**
- `pub fn greeting(hour: u32) -> &'static str` in `src/home.rs`.
- The home page with a 330 px tall area (still with a solid background) and the greeting for the
  local hour.
- "Recently played" as a 3-column grid with the 6 cards from `recents.tsv`: 44 px cover, title,
  subtitle and `···` in the accent color.
- "Mixed for you": a horizontally scrolling row of the cards from `mixes.tsv`, with 150 px covers.
- The whole home page inside a vertical `scrollable`.

**Hints:** `cargo add chrono` and `chrono::Local::now().hour()` (needs `use chrono::Timelike`).
Covers go through `image(image::Handle::from_bytes(...))`; for now you can read them with
`include_bytes!` or with `Handle::from_path`.

**Tests you need to write:**

| `greeting(h)` | Result |
|---|---|
| 4 | `"Good evening"` |
| 5 | `"Good morning"` |
| 11 | `"Good morning"` |
| 12 | `"Good afternoon"` |
| 19 | `"Good afternoon"` |
| 20 | `"Good evening"` |
| 23 | `"Good evening"` |

**Verification:** screenshot of the home page; `scripts/check.sh` passes.

- [x] **Commit:** `feat: add home greeting, recents and mixes`

### Step 12: blurred cover background

**Explanation:** `Result`, the `?` operator, and when to handle an error instead of panicking.
Working with pixels: decoding, shrinking and blurring.

**Reading:** `basic/result-error/result` (返回值 Result 和?), `basic/result-error/panic`
(panic! 深入剖析).

**Objective:** in `src/backdrop.rs`:
- `pub fn backdrop_size(width: u32, height: u32) -> (u32, u32)`: maximum width 480, never
  enlarging, keeping the aspect ratio.
- `pub fn blurred_backdrop(bytes: &[u8]) -> Option<image::Handle>`: decodes, shrinks with
  `backdrop_size`, blurs and returns a handle; `None` if the bytes are not an image.
- The home page's 330 px area shows the current track's cover blurred and cropped to fill
  (`ContentFit::Cover`), with a fade to `#19191c` at the bottom and the greeting on top. If
  `blurred_backdrop` returns `None`, the center color shows instead.
- The background is computed once per track and stored in the state, not on every `view`.
- Optional: a slow pan or zoom of the background driven by the clock tick.

**Hints:** `cargo add image@0.25 --no-default-features --features png`; `image::load_from_memory`,
`resize`, `imageops::fast_blur` (try a sigma between 20 and 40), `to_rgba8` and
`iced::widget::image::Handle::from_rgba`. To layer: `stack![background, fade, content]`; the fade is
a `container` with `Background::Gradient`.

**Tests you need to write:**

| Call | Result |
|---|---|
| `backdrop_size(1200, 1200)` | `(480, 480)` |
| `backdrop_size(960, 480)` | `(480, 240)` |
| `backdrop_size(300, 300)` | `(300, 300)` |
| `blurred_backdrop(b"this is not an image")` | `None` (review focus 4) |
| `blurred_backdrop` with a cover from `assets/covers/` | `Some(_)` |

**Verification:** screenshot of the home page with the background; `scripts/check.sh` passes.

- [x] **Commit:** `feat: blur the current cover behind home`

### Step 13: bottom player

**Explanation:** widgets with a value (`progress_bar`), aligning rows, and reusing a function that
builds icon buttons.

**Reading:** `basic/trait/generic` (泛型 Generics, to understand iced's signatures; you do not need
to write generics).

**Objective:** the player fixed at the bottom of the center, as in the mockup: a progress bar with
`format_time` on the left and the remaining time with a `-` on the right; shuffle, previous,
play/pause, next, repeat and like; the track card with a 40 px cover, title, artist and the
`OPUS 160` badge; volume, mini player and fullscreen.

**Hints:**
- `progress_bar(0.0..=duration as f32, elapsed as f32)` with height 4 and a custom style.
- `App` gets `liked: bool` and `Message::ToggleLike`; the thumb uses the accent color when active.
- `Playback` now knows which track is playing (`track: usize`) and takes the duration from `Track`.
- Buttons with no function: a `button(...)` without `on_press` is drawn disabled; give it a style
  that does not look gray.

**Verification:** screenshot of the player; play, pause and like work; `scripts/check.sh` passes.

- [x] **Commit:** `feat: add player bar`

### Step 14: next track

**Explanation:** the remainder operator (`%`) to wrap around a list, and updating several parts of
the state from a single message.

**Reading:** `basic/flow-control` (流程控制, review).

**Objective:** `Message::Next` moves to the next track in `tracks.tsv`: it changes the card, the
duration and the blurred background, and resets the clock to `0:00`. After the last track it goes
back to the first.

**Hints:** `pub fn next_index(current: usize, len: usize) -> usize` in `src/clock.rs`. The
background is recomputed in `update`, not in `view`.

**Tests you need to write:**

| `next_index(current, len)` | Result |
|---|---|
| `(0, 5)` | `1` |
| `(4, 5)` | `0` |
| `(0, 1)` | `0` |

**Verification:** pressing next changes the card and the background (two screenshots);
`scripts/check.sh` passes.

- [ ] **Commit:** `feat: skip to the next track`

### Step 15: lyrics panel

**Explanation:** parsing text with `split_once`, `strip_prefix` and `parse`; `let ... else`;
sorting a `Vec`; and showing a slice of the list (`&lines[a..b]`) without going out of range.

**Reading:** `basic/match-pattern/pattern-match` (模式适用场景), and a review of `string-slice`.

**Objective:** in `src/lyrics.rs`:
- `pub struct LyricLine { pub start: u32, pub text: String }` (`start` in seconds).
- `pub fn parse_lrc(text: &str) -> Vec<LyricLine>`: ignores metadata, empty lines and invalid
  times, and returns the lines sorted by `start`.
- `pub fn current_line(lines: &[LyricLine], elapsed: u32) -> Option<usize>`.
- The panel shows the 2 previous lines, the current one in `#f3f3f5` and slightly larger, and the
  next 6 dimmed. If the track has no lyrics, it shows "No lyrics for this track".

**Hints:** lyrics are parsed once, when the track changes. For the slice, use `saturating_sub` and
`min(len)`. The animated scrolling and the fade at the top and bottom are optional.

**Tests you need to write:**

| Input | Result |
|---|---|
| `parse_lrc("[01:10.00] Las calles")` | one line, `start` 70 |
| `parse_lrc("[ar:Los Satélites]")` | empty |
| `parse_lrc("[xx:10.00] hola")` | empty |
| `parse_lrc` with `[01:16.00] b` before `[01:10.00] a` | `a` first (focus 1) |
| `current_line(&lines, 69)` with the first at 70 | `None` |
| `current_line(&lines, 70)` | `Some(0)` |
| `current_line(&lines, 76)` with the second at 76 | `Some(1)` |
| `current_line(&lines, 999)` | the index of the last one |
| `current_line(&[], 50)` | `None` (focus 2) |

**Verification:** the lyrics follow the clock and the track without lyrics shows the message
(screenshots); `scripts/check.sh` passes.

- [ ] **Commit:** `feat: show synced lyrics`

### Step 16: font, narrow window and polish

**Explanation:** loading a font with `.font(...)` and `.default_font(...)`; listening to the window
size with a second subscription (`Subscription::batch`).

**Reading:** nothing new; a review of `closure`.

**Objective:**
- Inter as the font for the whole app (the assistant adds `assets/fonts/` with its OFL license).
- `pub fn visible_columns(width: f32, show_sidebar: bool, show_lyrics: bool) -> (bool, bool)` in
  `src/app.rs`: returns whether the sidebar and the lyrics are visible.
- Polish against the mockup: 1 px borders in the line color, corners, text sizes and hover.

**Hints:** `iced::window::resize_events()` yields `(Id, Size)`; store the width in the state. The
hide buttons still take effect when the window is wide.

**Tests you need to write:**

| `visible_columns(w, sidebar, lyrics)` | Result |
|---|---|
| `(1280.0, true, true)` | `(true, true)` |
| `(1099.0, true, true)` | `(true, false)` |
| `(719.0, true, true)` | `(false, false)` |
| `(1280.0, false, true)` | `(false, true)` |
| `(1099.0, true, false)` | `(true, false)` |

**Verification:** screenshots at full and narrow width; `scripts/check.sh` passes.

- [ ] **Commit:** `feat: load Inter and adapt to narrow windows`

### Step 17: closing phase 1

**Objective:**
- Compare the app with the mockup area by area; write down what is missing and decide whether to
  fix it now or move it to a later phase.
- Go through the spec's definition of done (section 6.6). That includes the user explaining in
  their own words what `Message`, `update`, `view` and the clock subscription do.
- `scripts/check.sh` green and a push to `origin/main`.

- [ ] **Push** when the phase is closed.
