//! Media digit helpers — typed constructors and parsers for media content.
//!
//! Media metadata is stored in Digit properties as `Value` types.
//! These helpers provide ergonomic creation and parsing of image, audio,
//! video, and stream digits without changing the Digit structure.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use x::Value;

// ---------------------------------------------------------------------------
// Meta types
// ---------------------------------------------------------------------------

/// Metadata for an image digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMeta {
    pub hash: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub blurhash: Option<String>,
    pub thumbnail_hash: Option<String>,
    pub alt: Option<String>,
}

/// Metadata for an audio digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioMeta {
    pub hash: String,
    pub mime: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub channels: u8,
    pub sample_rate: u32,
    pub codec: String,
}

/// Metadata for a video digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoMeta {
    pub chunks: Vec<String>,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub codec: String,
    pub thumbnail_hash: Option<String>,
    pub blurhash: Option<String>,
}

/// Kind of live stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    /// A music performance or DJ set.
    Music,
    /// A spoken conversation, podcast, or panel.
    Talk,
    /// A video broadcast.
    Video,
    /// A screen share.
    Screen,
}

/// Status of a live stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    /// Announced but not yet started.
    Scheduled,
    /// Currently broadcasting.
    Live,
    /// Broadcast has concluded.
    Ended,
}

/// Fortune configuration for a paid stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamFortuneConfig {
    pub tips_enabled: bool,
    pub ticket_price: Option<i64>,
    pub splits: Vec<(String, u32)>,
}

/// Metadata for a live stream digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamMeta {
    pub title: String,
    pub stream_kind: StreamKind,
    pub status: StreamStatus,
    pub relay_url: String,
    pub session_id: String,
    pub thumbnail_hash: Option<String>,
    pub fortune_config: Option<StreamFortuneConfig>,
}

// ---------------------------------------------------------------------------
// Digit constructors
// ---------------------------------------------------------------------------

/// Create an image digit from metadata.
pub fn image_digit(meta: &ImageMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("media.image".into(), Value::Null, author.into())?;
    digit = digit.with_property("hash".into(), Value::String(meta.hash.clone()), author);
    digit = digit.with_property("mime".into(), Value::String(meta.mime.clone()), author);
    digit = digit.with_property("width".into(), Value::Int(meta.width as i64), author);
    digit = digit.with_property("height".into(), Value::Int(meta.height as i64), author);
    digit = digit.with_property("size".into(), Value::Int(meta.size as i64), author);
    if let Some(ref bh) = meta.blurhash {
        digit = digit.with_property("blurhash".into(), Value::String(bh.clone()), author);
    }
    if let Some(ref th) = meta.thumbnail_hash {
        digit = digit.with_property("thumbnail-hash".into(), Value::String(th.clone()), author);
    }
    if let Some(ref alt) = meta.alt {
        digit = digit.with_property("alt".into(), Value::String(alt.clone()), author);
    }
    Ok(digit)
}

/// Create an audio digit from metadata.
pub fn audio_digit(meta: &AudioMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("media.audio".into(), Value::Null, author.into())?;
    digit = digit.with_property("hash".into(), Value::String(meta.hash.clone()), author);
    digit = digit.with_property("mime".into(), Value::String(meta.mime.clone()), author);
    digit = digit.with_property("duration".into(), Value::Double(meta.duration_secs), author);
    digit = digit.with_property("bitrate".into(), Value::Int(meta.bitrate as i64), author);
    digit = digit.with_property("channels".into(), Value::Int(meta.channels as i64), author);
    digit = digit.with_property("sample-rate".into(), Value::Int(meta.sample_rate as i64), author);
    digit = digit.with_property("codec".into(), Value::String(meta.codec.clone()), author);
    Ok(digit)
}

/// Create a video digit from metadata.
pub fn video_digit(meta: &VideoMeta, author: &str) -> Result<Digit, IdeasError> {
    let chunks_value = Value::Array(
        meta.chunks
            .iter()
            .map(|h| Value::String(h.clone()))
            .collect(),
    );
    let mut digit = Digit::new("media.video".into(), Value::Null, author.into())?;
    digit = digit.with_property("chunks".into(), chunks_value, author);
    digit = digit.with_property("mime".into(), Value::String(meta.mime.clone()), author);
    digit = digit.with_property("width".into(), Value::Int(meta.width as i64), author);
    digit = digit.with_property("height".into(), Value::Int(meta.height as i64), author);
    digit = digit.with_property("duration".into(), Value::Double(meta.duration_secs), author);
    digit = digit.with_property("bitrate".into(), Value::Int(meta.bitrate as i64), author);
    digit = digit.with_property("codec".into(), Value::String(meta.codec.clone()), author);
    if let Some(ref th) = meta.thumbnail_hash {
        digit = digit.with_property("thumbnail-hash".into(), Value::String(th.clone()), author);
    }
    if let Some(ref bh) = meta.blurhash {
        digit = digit.with_property("blurhash".into(), Value::String(bh.clone()), author);
    }
    Ok(digit)
}

/// Create a stream digit from metadata.
pub fn stream_digit(meta: &StreamMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("media.stream".into(), Value::Null, author.into())?;
    digit = digit.with_property("title".into(), Value::String(meta.title.clone()), author);
    digit = digit.with_property(
        "stream-kind".into(),
        Value::String(serde_json::to_string(&meta.stream_kind).unwrap_or_default().trim_matches('"').into()),
        author,
    );
    digit = digit.with_property(
        "status".into(),
        Value::String(serde_json::to_string(&meta.status).unwrap_or_default().trim_matches('"').into()),
        author,
    );
    digit = digit.with_property("relay-url".into(), Value::String(meta.relay_url.clone()), author);
    digit = digit.with_property("session-id".into(), Value::String(meta.session_id.clone()), author);
    if let Some(ref th) = meta.thumbnail_hash {
        digit = digit.with_property("thumbnail-hash".into(), Value::String(th.clone()), author);
    }
    if let Some(ref fc) = meta.fortune_config {
        let config_json = serde_json::to_string(fc).unwrap_or_default();
        digit = digit.with_property("fortune-config".into(), Value::String(config_json), author);
    }
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

fn prop_str(digit: &Digit, key: &str) -> Result<String, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| IdeasError::MediaParsing(format!("missing property: {key}")))
}

fn prop_str_opt(digit: &Digit, key: &str) -> Option<String> {
    digit.properties.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn prop_int(digit: &Digit, key: &str) -> Result<i64, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_int())
        .ok_or_else(|| IdeasError::MediaParsing(format!("missing property: {key}")))
}

fn prop_double(digit: &Digit, key: &str) -> Result<f64, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_double())
        .ok_or_else(|| IdeasError::MediaParsing(format!("missing property: {key}")))
}

/// Parse image metadata from a digit.
pub fn parse_image_meta(digit: &Digit) -> Result<ImageMeta, IdeasError> {
    if digit.digit_type() != "media.image" {
        return Err(IdeasError::MediaParsing(format!(
            "expected media.image, got {}",
            digit.digit_type()
        )));
    }
    Ok(ImageMeta {
        hash: prop_str(digit, "hash")?,
        mime: prop_str(digit, "mime")?,
        width: prop_int(digit, "width")? as u32,
        height: prop_int(digit, "height")? as u32,
        size: prop_int(digit, "size")? as u64,
        blurhash: prop_str_opt(digit, "blurhash"),
        thumbnail_hash: prop_str_opt(digit, "thumbnail-hash"),
        alt: prop_str_opt(digit, "alt"),
    })
}

/// Parse audio metadata from a digit.
pub fn parse_audio_meta(digit: &Digit) -> Result<AudioMeta, IdeasError> {
    if digit.digit_type() != "media.audio" {
        return Err(IdeasError::MediaParsing(format!(
            "expected media.audio, got {}",
            digit.digit_type()
        )));
    }
    Ok(AudioMeta {
        hash: prop_str(digit, "hash")?,
        mime: prop_str(digit, "mime")?,
        duration_secs: prop_double(digit, "duration")?,
        bitrate: prop_int(digit, "bitrate")? as u32,
        channels: prop_int(digit, "channels")? as u8,
        sample_rate: prop_int(digit, "sample-rate")? as u32,
        codec: prop_str(digit, "codec")?,
    })
}

/// Parse video metadata from a digit.
pub fn parse_video_meta(digit: &Digit) -> Result<VideoMeta, IdeasError> {
    if digit.digit_type() != "media.video" {
        return Err(IdeasError::MediaParsing(format!(
            "expected media.video, got {}",
            digit.digit_type()
        )));
    }
    let chunks = digit
        .properties
        .get("chunks")
        .and_then(|v| {
            if let Value::Array(arr) = v {
                Some(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            } else {
                None
            }
        })
        .ok_or_else(|| IdeasError::MediaParsing("missing property: chunks".into()))?;

    Ok(VideoMeta {
        chunks,
        mime: prop_str(digit, "mime")?,
        width: prop_int(digit, "width")? as u32,
        height: prop_int(digit, "height")? as u32,
        duration_secs: prop_double(digit, "duration")?,
        bitrate: prop_int(digit, "bitrate")? as u32,
        codec: prop_str(digit, "codec")?,
        thumbnail_hash: prop_str_opt(digit, "thumbnail-hash"),
        blurhash: prop_str_opt(digit, "blurhash"),
    })
}

/// Parse stream metadata from a digit.
pub fn parse_stream_meta(digit: &Digit) -> Result<StreamMeta, IdeasError> {
    if digit.digit_type() != "media.stream" {
        return Err(IdeasError::MediaParsing(format!(
            "expected media.stream, got {}",
            digit.digit_type()
        )));
    }
    let stream_kind_str = prop_str(digit, "stream-kind")?;
    let stream_kind: StreamKind = serde_json::from_str(&format!("\"{stream_kind_str}\""))
        .map_err(|e| IdeasError::MediaParsing(format!("invalid stream-kind: {e}")))?;

    let status_str = prop_str(digit, "status")?;
    let status: StreamStatus = serde_json::from_str(&format!("\"{status_str}\""))
        .map_err(|e| IdeasError::MediaParsing(format!("invalid status: {e}")))?;

    let fortune_config = prop_str_opt(digit, "fortune-config")
        .and_then(|s| serde_json::from_str(&s).ok());

    Ok(StreamMeta {
        title: prop_str(digit, "title")?,
        stream_kind,
        status,
        relay_url: prop_str(digit, "relay-url")?,
        session_id: prop_str(digit, "session-id")?,
        thumbnail_hash: prop_str_opt(digit, "thumbnail-hash"),
        fortune_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image_meta() -> ImageMeta {
        ImageMeta {
            hash: "abc123".into(),
            mime: "image/png".into(),
            width: 1920,
            height: 1080,
            size: 2_500_000,
            blurhash: Some("LEHV6nWB2yk8".into()),
            thumbnail_hash: Some("thumb123".into()),
            alt: Some("A sunset over the ocean".into()),
        }
    }

    fn test_audio_meta() -> AudioMeta {
        AudioMeta {
            hash: "audio456".into(),
            mime: "audio/opus".into(),
            duration_secs: 245.3,
            bitrate: 128_000,
            channels: 2,
            sample_rate: 48_000,
            codec: "opus".into(),
        }
    }

    fn test_video_meta() -> VideoMeta {
        VideoMeta {
            chunks: vec!["chunk1".into(), "chunk2".into(), "chunk3".into()],
            mime: "video/mp4".into(),
            width: 1920,
            height: 1080,
            duration_secs: 120.5,
            bitrate: 5_000_000,
            codec: "h264".into(),
            thumbnail_hash: Some("vthumb789".into()),
            blurhash: Some("LKO2?U%2Tw=w".into()),
        }
    }

    fn test_stream_meta() -> StreamMeta {
        StreamMeta {
            title: "Live Jazz Session".into(),
            stream_kind: StreamKind::Music,
            status: StreamStatus::Live,
            relay_url: "wss://relay.example.com".into(),
            session_id: "session-001".into(),
            thumbnail_hash: None,
            fortune_config: Some(StreamFortuneConfig {
                tips_enabled: true,
                ticket_price: Some(100),
                splits: vec![("cpub1artist".into(), 80), ("cpub1venue".into(), 20)],
            }),
        }
    }

    #[test]
    fn image_digit_round_trip() {
        let meta = test_image_meta();
        let digit = image_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "media.image");

        let parsed = parse_image_meta(&digit).unwrap();
        assert_eq!(parsed.hash, meta.hash);
        assert_eq!(parsed.mime, meta.mime);
        assert_eq!(parsed.width, meta.width);
        assert_eq!(parsed.height, meta.height);
        assert_eq!(parsed.size, meta.size);
        assert_eq!(parsed.blurhash, meta.blurhash);
        assert_eq!(parsed.thumbnail_hash, meta.thumbnail_hash);
        assert_eq!(parsed.alt, meta.alt);
    }

    #[test]
    fn image_digit_minimal() {
        let meta = ImageMeta {
            hash: "h".into(),
            mime: "image/jpeg".into(),
            width: 100,
            height: 100,
            size: 5000,
            blurhash: None,
            thumbnail_hash: None,
            alt: None,
        };
        let digit = image_digit(&meta, "bob").unwrap();
        let parsed = parse_image_meta(&digit).unwrap();
        assert_eq!(parsed.hash, "h");
        assert!(parsed.blurhash.is_none());
        assert!(parsed.alt.is_none());
    }

    #[test]
    fn audio_digit_round_trip() {
        let meta = test_audio_meta();
        let digit = audio_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "media.audio");

        let parsed = parse_audio_meta(&digit).unwrap();
        assert_eq!(parsed.hash, meta.hash);
        assert_eq!(parsed.mime, meta.mime);
        assert_eq!(parsed.duration_secs, meta.duration_secs);
        assert_eq!(parsed.bitrate, meta.bitrate);
        assert_eq!(parsed.channels, meta.channels);
        assert_eq!(parsed.sample_rate, meta.sample_rate);
        assert_eq!(parsed.codec, meta.codec);
    }

    #[test]
    fn video_digit_round_trip() {
        let meta = test_video_meta();
        let digit = video_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "media.video");

        let parsed = parse_video_meta(&digit).unwrap();
        assert_eq!(parsed.chunks, meta.chunks);
        assert_eq!(parsed.mime, meta.mime);
        assert_eq!(parsed.width, meta.width);
        assert_eq!(parsed.height, meta.height);
        assert_eq!(parsed.duration_secs, meta.duration_secs);
        assert_eq!(parsed.codec, meta.codec);
        assert_eq!(parsed.thumbnail_hash, meta.thumbnail_hash);
        assert_eq!(parsed.blurhash, meta.blurhash);
    }

    #[test]
    fn stream_digit_round_trip() {
        let meta = test_stream_meta();
        let digit = stream_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "media.stream");

        let parsed = parse_stream_meta(&digit).unwrap();
        assert_eq!(parsed.title, meta.title);
        assert_eq!(parsed.stream_kind, StreamKind::Music);
        assert_eq!(parsed.status, StreamStatus::Live);
        assert_eq!(parsed.relay_url, meta.relay_url);
        assert_eq!(parsed.session_id, meta.session_id);

        let fc = parsed.fortune_config.unwrap();
        assert!(fc.tips_enabled);
        assert_eq!(fc.ticket_price, Some(100));
        assert_eq!(fc.splits.len(), 2);
    }

    #[test]
    fn stream_digit_no_fortune() {
        let meta = StreamMeta {
            title: "Free Talk".into(),
            stream_kind: StreamKind::Talk,
            status: StreamStatus::Scheduled,
            relay_url: "wss://r.com".into(),
            session_id: "s-002".into(),
            thumbnail_hash: None,
            fortune_config: None,
        };
        let digit = stream_digit(&meta, "bob").unwrap();
        let parsed = parse_stream_meta(&digit).unwrap();
        assert_eq!(parsed.stream_kind, StreamKind::Talk);
        assert_eq!(parsed.status, StreamStatus::Scheduled);
        assert!(parsed.fortune_config.is_none());
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_image_meta(&digit).is_err());
        assert!(parse_audio_meta(&digit).is_err());
        assert!(parse_video_meta(&digit).is_err());
        assert!(parse_stream_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("media.image".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_image_meta(&digit).is_err());
    }

    #[test]
    fn serde_round_trip() {
        let meta = test_image_meta();
        let digit = image_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_image_meta(&rt).unwrap();
        assert_eq!(parsed.hash, meta.hash);
        assert_eq!(parsed.width, meta.width);
    }

    #[test]
    fn meta_types_serde() {
        let img = test_image_meta();
        let json = serde_json::to_string(&img).unwrap();
        let rt: ImageMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.hash, img.hash);

        let aud = test_audio_meta();
        let json = serde_json::to_string(&aud).unwrap();
        let rt: AudioMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.codec, aud.codec);

        let vid = test_video_meta();
        let json = serde_json::to_string(&vid).unwrap();
        let rt: VideoMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.chunks.len(), 3);

        let strm = test_stream_meta();
        let json = serde_json::to_string(&strm).unwrap();
        let rt: StreamMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.stream_kind, StreamKind::Music);
    }
}
