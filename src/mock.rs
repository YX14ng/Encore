//! Sample data for phase 1, read from the files in `assets/mock/`.
use iced::Color;

/// A track that can be played.
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    #[allow(dead_code)]
    pub cover: String,
    #[allow(dead_code)]
    pub lyrics: Option<String>,
}

/// Reads `tracks.tsv`: one track per line, six tab-separated fields.
/// Lines that are empty or malformed are skipped.
pub fn parse_tracks(text: &str) -> Vec<Track> {
    text.lines().filter_map(parse_track).collect()
}

fn parse_track(line: &str) -> Option<Track> {
    let mut fields = line.split('\t');
    let title = fields.next()?;
    let artist = fields.next()?;
    let album = fields.next()?;
    let duration = fields.next()?.parse().ok()?;
    let cover = fields.next()?;
    let lyrics = fields.next()?;
    if fields.next().is_some() {
        return None;
    }

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        duration,
        cover: cover.to_string(),
        lyrics: (lyrics != "-").then(|| lyrics.to_string()),
    })
}

/// A playlist in the sidebar, drawn with a colored square.
pub struct Playlist {
    pub name: String,
    pub color: Color,
}

/// Reads `playlists.tsv`: a name and a `#rrggbb` color per line.
pub fn parse_playlists(text: &str) -> Vec<Playlist> {
    text.lines().filter_map(parse_playlist).collect()
}

fn parse_playlist(line: &str) -> Option<Playlist> {
    let (name, color) = line.split_once('\t')?;
    Some(Playlist {
        name: name.to_string(),
        color: color.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_line() {
        let tracks =
            parse_tracks("Luz de neón\tLos Satélites\tCiudad dormida\t214\tluz.png\tluz.lrc");
        assert_eq!(tracks.len(), 1);
        let track = &tracks[0];
        assert_eq!(track.title, "Luz de neón");
        assert_eq!(track.artist, "Los Satélites");
        assert_eq!(track.album, "Ciudad dormida");
        assert_eq!(track.duration, 214);
        assert_eq!(track.cover, "luz.png");
        assert_eq!(track.lyrics.as_deref(), Some("luz.lrc"));
    }

    #[test]
    fn a_dash_means_no_lyrics() {
        let tracks = parse_tracks("Route 40\tLos Satélites\tCiudad dormida\t198\troute.png\t-");
        assert_eq!(tracks[0].lyrics, None);
    }

    #[test]
    fn skips_lines_with_missing_fields() {
        assert!(parse_tracks("Route 40\tLos Satélites\tCiudad dormida\t198\troute.png").is_empty());
    }

    #[test]
    fn skips_lines_with_extra_fields() {
        assert!(parse_tracks("a\tb\tc\t1\td\t-\textra").is_empty());
    }

    #[test]
    fn skips_lines_with_a_bad_duration() {
        assert!(
            parse_tracks("Route 40\tLos Satélites\tCiudad dormida\tabc\troute.png\t-").is_empty()
        );
    }

    #[test]
    fn ignores_empty_lines() {
        let text = "a\tb\tc\t1\td\t-\n\n\na\tb\tc\t2\td\t-\n";
        assert_eq!(parse_tracks(text).len(), 2);
    }

    #[test]
    fn reads_the_sample_file() {
        let tracks = parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        assert_eq!(tracks.len(), 5);
        assert_eq!(tracks[0].title, "Luz de neón");
    }

    #[test]
    fn parses_a_playlist_with_its_color() {
        let playlists = parse_playlists("Study\t#ff5f6d");
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Study");
        assert_eq!(playlists[0].color, Color::from_rgb8(0xff, 0x5f, 0x6d));
    }

    #[test]
    fn skips_playlists_with_a_bad_color() {
        assert!(parse_playlists("Study\tred-ish").is_empty());
        assert!(parse_playlists("Study").is_empty());
    }

    #[test]
    fn reads_the_sample_playlists() {
        assert_eq!(
            parse_playlists(include_str!("../assets/mock/playlists.tsv")).len(),
            5
        );
    }
}
