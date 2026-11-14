//! SponsorBlock: the community marks the parts of music videos that are not music (intros,
//! skits, outros) and Encore jumps over them. The video is looked up by the first characters of
//! its SHA-256 hash, so the server never learns which video is playing.

use serde_json::Value;
use sha2::{Digest, Sha256};

const API_URL: &str = "https://sponsor.ajay.app/api/skipSegments";
const CATEGORIES: &str = r#"["music_offtopic"]"#;
/// Segments are only skipped while more than this much of them is left, so a jump that lands
/// a little early does not loop.
const MIN_LEFT: f64 = 1.0;

/// A part of a video to skip, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
}

/// The parts of `video_id` to skip. No segments is the usual answer (most songs have none).
pub async fn fetch(video_id: String) -> Result<Vec<Segment>, String> {
    let entries = fetch_bucket(&hash_prefix(&video_id)).await?;
    Ok(segments_for(&entries, &video_id))
}

/// Every video whose hash starts like `prefix`, as SponsorBlock answers.
async fn fetch_bucket(prefix: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .get(format!("{API_URL}/{prefix}"))
        .query(&[("categories", CATEGORIES)])
        .header(
            reqwest::header::USER_AGENT,
            "Encore (desktop YouTube Music player)",
        )
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Value::Array(Vec::new()));
    }
    let response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    response.json().await.map_err(|error| error.to_string())
}

/// The first four hex digits of the video id's SHA-256.
pub fn hash_prefix(video_id: &str) -> String {
    Sha256::digest(video_id.as_bytes())
        .iter()
        .take(2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The skip segments of `video_id` among a bucket's entries, sorted by start.
pub fn segments_for(entries: &Value, video_id: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = entries
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("videoID").and_then(Value::as_str) == Some(video_id))
        .filter_map(|entry| entry.get("segments")?.as_array())
        .flatten()
        .filter(|segment| {
            segment
                .get("actionType")
                .and_then(Value::as_str)
                .unwrap_or("skip")
                == "skip"
        })
        .filter_map(|segment| {
            let times = segment.get("segment")?.as_array()?;
            let start = times.first()?.as_f64()?;
            let end = times.get(1)?.as_f64()?;
            (end > start).then_some(Segment { start, end })
        })
        .collect();
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

/// Where to jump at `position`: the end of the segment it falls in, if any.
pub fn skip_target(segments: &[Segment], position: f64) -> Option<f64> {
    segments
        .iter()
        .find(|segment| segment.start <= position && position < segment.end - MIN_LEFT)
        .map(|segment| segment.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_the_id_like_sponsorblock() {
        // `printf %s dQw4w9WgXcQ | sha256sum` starts with "5f6b".
        assert_eq!(hash_prefix("dQw4w9WgXcQ"), "5f6b");
    }

    #[test]
    fn keeps_only_this_videos_skip_segments() {
        let bucket: Value = serde_json::from_str(
            r#"[
                {"videoID": "other", "segments": [{"segment": [0, 10], "actionType": "skip"}]},
                {"videoID": "abc", "segments": [
                    {"segment": [200.5, 230.0], "actionType": "skip"},
                    {"segment": [0.0, 12.0], "actionType": "skip"},
                    {"segment": [50.0, 60.0], "actionType": "mute"}
                ]}
            ]"#,
        )
        .expect("json");
        assert_eq!(
            segments_for(&bucket, "abc"),
            vec![
                Segment {
                    start: 0.0,
                    end: 12.0
                },
                Segment {
                    start: 200.5,
                    end: 230.0
                },
            ]
        );
    }

    #[test]
    fn jumps_to_the_end_of_the_segment_it_is_in() {
        let segments = [Segment {
            start: 5.0,
            end: 12.0,
        }];
        assert_eq!(skip_target(&segments, 4.9), None);
        assert_eq!(skip_target(&segments, 5.0), Some(12.0));
        assert_eq!(skip_target(&segments, 11.5), None, "too close to the end");
    }

    /// Asks SponsorBlock for a bucket and checks some of its music videos have segments:
    /// `cargo test -- --ignored reads_real_segments`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn reads_real_segments() {
        let bucket = fetch_bucket(&hash_prefix("dQw4w9WgXcQ"))
            .await
            .expect("answer");
        let ids: Vec<&str> = bucket
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|entry| entry.get("videoID")?.as_str())
            .collect();
        let with_segments = ids
            .iter()
            .filter(|id| !segments_for(&bucket, id).is_empty())
            .count();
        assert!(with_segments > 0, "no segments among {} videos", ids.len());
    }
}
