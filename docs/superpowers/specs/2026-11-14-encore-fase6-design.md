# Encore — phase 6: audio extras

Status: implemented on 2026-11-14 (`e238a93`, `a20f612`).

## Goal

Make the player bar fully work and add the audio features Cider and Pear offer: an equalizer,
volume normalization, playback speed and crossfade.

## What was built

### Player bar (`player.rs`, `queue.rs`, `audio_settings.rs`)

- **Seek:** the progress bar is a slim slider; dragging shows the target time and the jump
  happens on release (`Message::Seeking` / `SeekDone`).
- **Volume:** slider plus mute on the speaker icon. mpv's volume scale is cubic, so the slider maps
  straight onto it.
- **Previous:** restarts the song after 3 seconds, otherwise goes to the previous song (wrapping
  with repeat all). Also on the MPRIS "previous" key.
- **Shuffle:** `Queue` keeps the list order and a play order; shuffling puts the playing (or
  picked) song first and the rest in random order (`fastrand`). Turning it off returns to list
  order from the playing song.
- **Repeat:** off → all → one. Repeat one replays the song when it ends; the next button still
  moves on.
- Volume, mute, shuffle and repeat survive restarts in `~/.config/encore/audio.json`.

### Audio screen (`audio_view.rs`, opened with the equalizer button in the player bar)

- **Equalizer:** 10 bands (31 Hz–16 kHz, ±12 dB, octave-wide peaking filters) and 7 presets.
  A preamp of minus the largest boost keeps boosts from clipping.
- **Normalize volume:** `loudnorm` to -14 LUFS (resampled back to 48 kHz).
- **Playback speed:** 0.5×–2×, optionally keeping the pitch (`audio-pitch-correction`).
- **Crossfade:** 0–12 s.
- Everything is one mpv `af` value, `lavfi=[...]`, built by `AudioSettings::filter_chain`.
  Gain and crossfade sliders apply on release; speed follows the drag.

### Crossfade (`mixer.rs`)

- Two mpv instances ("decks"). When the playing song is `crossfade` seconds (in real time, so
  divided by the speed) from its end and the next song is already downloaded, the next song starts
  on the other deck and the volumes follow an equal-power curve (cube roots, because mpv's volume
  is cubic), stepped on every animation frame.
- Pausing or seeking during a fade finishes it at once. No crossfade with repeat one.

## Verification

- Unit tests: queue order, shuffle, wrap, repeat cycle; settings clamping, presets, filter chain;
  equal-power fade curve.
- Ignored tests against real mpv with muted output: `seeks_in_a_song` and `mixes_real_songs`
  (equalizer + normalization + 2× speed accepted by mpv, then a 1 s crossfade that stops the old
  song).
- Offscreen snapshot of the audio screen (iced_test, outside the repo).

## Left for later

- The mini player button and the static "OPUS 160" badge.
- Gapless playback without crossfade (each song still starts after the previous one ends).
