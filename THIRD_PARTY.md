# Third-party material

Encore's own code is licensed under the GNU General Public License, version 3 or (at your
option) any later version; see `LICENSE`. The built program links RustyPipe, which is
GPL-3.0-only, so the program as a whole is distributed under the GPL version 3. The parts below
come from others and keep their own licenses.

## Bundled in the source and the binary

| What | Where | License |
|---|---|---|
| Inter typeface, by Rasmus Andersson | `assets/fonts/` | SIL Open Font License 1.1, `assets/fonts/OFL.txt` |
| IPADIC Japanese dictionary (NAIST, ICOT), for romanized lyrics; downloaded while building and embedded by `lindera-ipadic` | in the binary | `licenses/IPADIC.txt` |

The icons in `assets/icons/` and the sample data in `assets/mock/` were made for Encore and
are under Encore's license.

## Rust libraries

Encore is built from crates.io libraries and the `encore` branch of
[YX14ng/rustypipe](https://github.com/YX14ng/rustypipe), a fork of
[RustyPipe](https://codeberg.org/ThetaDev/rustypipe) (GPL-3.0). Each library's license is in
its own package; `cargo tree --format "{p} {l}"` lists them. Besides RustyPipe (GPL-3.0-only)
and the libmpv bindings (`libmpv2`, LGPL-2.1), they are under permissive licenses (MIT,
Apache-2.0, BSD, ISC, Zlib, BSL-1.0, Unicode-3.0, CC0-1.0, Unlicense), all of which can be
combined with the GPL version 3.

## Programs Encore runs

Encore starts these programs if they are installed; it does not include them:
[mpv](https://mpv.io) (through libmpv; GPL-2.0-or-later or LGPL-2.1-or-later, depending on how
it was built), [yt-dlp](https://github.com/yt-dlp/yt-dlp) (Unlicense) and
[FFmpeg](https://ffmpeg.org) (LGPL-2.1-or-later or GPL-2.0-or-later, depending on how it was
built). A package that bundles them has to ship their licenses too.
