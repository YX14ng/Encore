# Encore — phase 7: themes and plugins

Status: implemented on 2026-11-14 (`ae5c653`, `2883a2c`, `305b4a9`).

## Goal

Let the user change how Encore looks (like Cider's themes and accent colors) and switch optional
features on and off (like Pear's plugins).

## Settings screen

A gear in the top bar opens Settings with three tabs: Appearance, Audio (the phase 6 screen,
also opened by the equalizer button) and Plugins.

## Themes (`theme.rs`, `themes.rs`, `settings_view.rs`)

- Colors are a `Palette` read at draw time from a global (`theme::palette()`), because iced's
  style closures cannot receive it; only the appearance settings write it. The refactor kept the
  interface pixel-identical (checked with offscreen snapshots).
- Built-in themes: Encore, Midnight, OLED, Nord and Daylight (light; iced's own widgets switch
  to their light look).
- Custom themes: JSON files in `~/.config/encore/themes/` with a name and `#rrggbb` colors;
  missing colors come from Encore (or Daylight with `"light": true`). Broken files are listed on
  the screen with the reason. "Reload themes" rereads the folder.
- Accent color: the theme's, one of 8 fixed colors, or "from cover": the strongest hue of the
  playing song's cover (hue histogram weighted by saturation × brightness), lifted so it reads
  on the background.
- Choices are kept in `~/.config/encore/appearance.json`.

## Plugins (`plugins.rs`, `notifications.rs`, `sponsorblock.rs`)

Kept in `~/.config/encore/plugins.json`:

- **Discord Rich Presence** (on by default): switching it off clears the status.
- **Last.fm scrobbling** (on): off means no "now playing" and no scrobbles.
- **Desktop notifications** (off): title, "artist — album" and the cover through the
  freedesktop notification service (`notify-rust`, zbus); each one replaces the previous.
- **SponsorBlock** (on): fetches the `music_offtopic` segments of the playing video by the
  first four hex digits of its SHA-256 (the server never learns the video) and seeks past them.

## Verification

- Unit tests: hex colors, theme files, accent choice, cover color, HSV; plugin switches and old
  files; notification text; SponsorBlock hash prefix (checked against `sha256sum`), segment
  filtering and skip targets.
- Ignored network test `reads_real_segments` against the real SponsorBlock API.
- Offscreen snapshots of the appearance and plugins tabs and of Home in Daylight, Midnight and
  with the cover accent.
- Not checked: a real desktop notification (it would pop up on the user's screen).
