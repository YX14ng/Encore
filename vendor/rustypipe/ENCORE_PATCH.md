# Encore's patch to rustypipe 0.11.4

This is rustypipe 0.11.4 from crates.io (GPL-3.0, see LICENSE) with one change, used through
`[patch.crates-io]` in Encore's `Cargo.toml`:

- `src/client/response/music_playlist.rs`: `Header` also reads the
  `musicEditablePlaylistDetailHeaderRenderer` that YouTube Music wraps around the header of the
  signed-in user's own playlists. Without it, `music_playlist` treats those playlists as albums
  and fails with "album playlist containing items from different albums".

Drop this folder once a rustypipe release reads that header.
