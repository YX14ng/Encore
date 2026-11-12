# Encore

A desktop player for YouTube Music, written in native Rust with [iced](https://iced.rs).

The interface takes its cues from Cider: a sidebar with the library, a home page with a background
image and a grid of recent plays, a built-in player at the bottom, and a synced lyrics panel on the
right.

## Status

Phase 1: building the visual skeleton with sample data.

- Design: [`docs/superpowers/specs/2026-11-12-encore-fase1-design.md`](docs/superpowers/specs/2026-11-12-encore-fase1-design.md)
- Step plan: [`docs/superpowers/plans/2026-11-12-encore-fase1.md`](docs/superpowers/plans/2026-11-12-encore-fase1.md)

## Planned stack

- Rust (2024 edition)
- iced for the interface
- YouTube Music internal API client and audio engine: to be decided in the design
