# Encore — keeping the YouTube Music session alive

Date: 2026-11-15
Status: researched, not built

## What happens today

- Signing in happens in Encore's own WebKitGTK window with an ephemeral context; the cookies of
  `music.youtube.com` go to rustypipe as one `Cookie` header and are stored in
  `~/.local/share/encore/rustypipe/rustypipe_cache.json`.
- rustypipe keeps that header as fixed text: it ignores the `Set-Cookie` headers YouTube sends
  back, and it never renews anything. yt-dlp (High quality) gets a copy of the same header per
  download and does not renew it either.
- When Google stops accepting the session, YouTube answers 401 and Encore says the session
  expired (`playlists::edit::SESSION_EXPIRED`); the library falls back to the cached copy.

## Why sessions died in minutes before

Google rotates `__Secure-1PSIDTS` (and the `*SIDCC` cookies) while a browser has the session
open; a copy taken from that browser is superseded and stops working soon after (yt-dlp warns
"the provided YouTube account cookies are no longer valid … likely rotated in the browser",
yt-dlp #8227 / PR #13014). Cookies copied from Zen died within seconds to minutes for that
reason. yt-dlp's wiki prescribes exactly what Encore's sign-in window already does: sign in
in a private session and never open that session in a browser again.

Observed: the session from the sign-in window of 2026-11-14 ~18:20 still worked on
2026-11-15 19:00 (the signed-in library loaded), about 25 hours with no renewal at all.

## How a browser renews the session

`POST https://accounts.google.com/RotateCookies` with the account's cookies,
`Content-Type: application/json`, `Origin: https://accounts.google.com` and the fixed body
`[000,"-0000000000000000000"]`. The answer sets a new `__Secure-1PSIDTS` (plus related cookies)
and its body carries the suggested interval (`["identity.hfcr",600]`, ten minutes). Projects
that keep Google web sessions alive (gemini-webapi: `utils/rotate_1psidts.py`, about every
9 minutes; notebooklm-py: keepalive every 600 s) do exactly this, never more than once a minute
(Google answers 429), and treat 401 as "sign in again".

Unknowns that only a live test answers:
- whether the `.youtube.com` copies of the cookies stay valid after rotating on `.google.com`
  (browsers also rotate YouTube's copy through `accounts.youtube.com`), and
- how long an Encore-only session lives with no rotation at all (days, weeks).

A wrong rotation is not harmless: once Google issues a new `__Secure-1PSIDTS`, the old one is
superseded, so a rotation whose answer is not stored signs Encore out.

## Plan

1. **Measure first, no risk.** Record when the session was created (sign-in time in Encore's
   data dir) and, whenever YouTube answers 401, how old it was. Show "Signed in N days ago" on
   the account screen.
2. **Make expiry painless.** On a 401, show one button that opens the sign-in window again
   instead of only an error; keep the cached library meanwhile (already done).
3. **Rotate, only if 1 shows sessions dying.** In the rustypipe fork: let the auth cookie be
   updated from `Set-Cookie` answers and expose a `user_auth_rotate_cookie()` that posts
   `RotateCookies`, merges the new cookies into the stored header and saves it. Encore calls it
   at start and every 10 minutes while open, at most once a minute, and stops on 401. The sign-in
   window must then also keep the `.google.com` cookies. Testing it touches the user's real
   session, so it needs the user's go-ahead, and a failed test means signing in again.

## Sources

- yt-dlp wiki, "Exporting YouTube cookies"; yt-dlp issue #8227 and PR #13014.
- gemini-webapi (`HanaokaYuzu/Gemini-API`): `src/gemini_webapi/utils/rotate_1psidts.py`,
  `constants.py` (`Endpoint.ROTATE_COOKIES`, `Headers.ROTATE_COOKIES`).
- notebooklm-py, `docs/auth-cookie-lifecycle.md` (rotation interval, supersede timing).
