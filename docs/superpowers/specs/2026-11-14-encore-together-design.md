# Encore — Listen together

Listening along with friends, like Cider Together, without a server of Encore's own: one Encore
hosts a session and the friends' Encore connect straight to it. Discord carries the invitation.

## How a session works

- **Host.** Opens a TCP port (41730 by default) and makes an *invite*:
  `encore:<address>:<port>:<token>`. The address is the one friends use to reach the host: the
  public IP (found through `api.ipify.org` when hosting starts) or one the user types. The token
  is 16 random bytes from `/dev/urandom`, as hex.
- **Guests.** Connect to the address, send the token and their name, and get the host's state:
  the song (its YouTube id, title, artist, album, length and cover address), the position and
  whether it plays. Each guest downloads the song on its own with yt-dlp, like any song, and
  follows the host: same song, position within 2 seconds, paused when the host pauses.
- **Only the host chooses.** A guest's own controls keep working, but the host's next state wins.
  Leaving the session returns the guest's player to normal.
- **Over the internet** the host's router must forward the port (as with any game server).

## Discord

When the Discord plugin is on and a session is hosted, the presence carries a party (the session,
its size and a limit of 8 listeners) and the invite as the *join secret*. Friends see "Ask to
Join" on the host's profile, or get an invite the host sends from a chat's "+" menu.

- A friend's Encore subscribes to `ACTIVITY_JOIN`; clicking an invite gives it the join secret,
  which is the invite: it joins.
- The host's Encore subscribes to `ACTIVITY_JOIN_REQUEST`; requests show on the together screen
  with Accept and Decline (`SEND_ACTIVITY_JOIN_INVITE` / `CLOSE_ACTIVITY_REQUEST`).
- This needs reading Discord's socket, which `discord-rich-presence` cannot do, so Encore speaks
  the IPC protocol itself (frames of opcode, length and JSON) and keeps that crate's activity
  types for the JSON.
- The invite can also be copied and pasted by hand, so a session works without Discord.

## Protocol

One JSON object per line, at most 16 KiB. Guest → host: `hello {token, name}`. Host → guest:
`welcome {host}`, `refused {reason}`, `state {track, position_ms, playing}`. The host sends a
state when the song changes, on play or pause, on a seek, and every 5 seconds.

## Safety

- The port is opened only while hosting. A connection that does not send the right token within
  5 seconds is closed; at most 8 guests.
- A guest checks everything it gets: the video id must look like one (11 characters of
  `A-Z a-z 0-9 _ -`) because it ends up in a yt-dlp address and a cache file name; texts are cut
  to 200 characters; the cover must be an `https://` address. Anything else is ignored.
- Guests learn the host's address (they connect to it); the host learns the guests' addresses.
  The together screen says so.

## Screens

The together button in the top bar opens the together screen: host a session (address and port,
the invite with a copy button, the listeners and Discord join requests) or join one by pasting an
invite. The button is highlighted while a session runs. The cast button is gone: there is no
device to cast to.
