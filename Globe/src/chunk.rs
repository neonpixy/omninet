use std::collections::HashSet;

use crown::CrownKeypair;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// Default chunk size: 1 MB.
pub const DEFAULT_CHUNK_SIZE: u64 = 1_048_576;

/// Information about a single chunk in a manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkInfo {
    /// SHA-256 hex hash of this chunk's bytes.
    pub hash: String,
    /// Size of this chunk in bytes.
    pub size: u64,
    /// Zero-based position in the sequence.
    pub index: u32,
}

/// A manifest describing a large file split into content-addressed chunks.
///
/// Each chunk is a regular asset in AssetStore, individually uploaded via
/// `PUT /asset/{hash}`. The manifest ties them together as an ordered
/// sequence and records the SHA-256 of the reassembled file for verification.
///
/// Videos, large images, firmware updates — anything too big for a single
/// blob upload uses this format. Download is resumable: fetch only the
/// chunks you're missing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkManifest {
    /// SHA-256 hex hash of the fully reassembled file.
    pub content_hash: String,
    /// Total size of the reassembled file in bytes.
    pub total_size: u64,
    /// Size of each chunk (last chunk may be smaller).
    pub chunk_size: u64,
    /// Ordered list of chunks.
    pub chunks: Vec<ChunkInfo>,
}

/// Builds and parses chunk manifest events (kind 9000).
pub struct ChunkBuilder;

impl ChunkBuilder {
    /// Split bytes into chunks, compute their hashes, and build a manifest.
    pub fn split(data: &[u8], chunk_size: u64) -> ChunkManifest {
        let chunk_size = if chunk_size == 0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size
        };

        let mut content_hasher = Sha256::new();
        content_hasher.update(data);
        let content_hash = hex::encode(content_hasher.finalize());

        let mut chunks = Vec::new();
        let mut offset = 0usize;
        let mut index = 0u32;

        while offset < data.len() {
            let end = (offset + chunk_size as usize).min(data.len());
            let chunk_data = &data[offset..end];

            let mut hasher = Sha256::new();
            hasher.update(chunk_data);
            let hash = hex::encode(hasher.finalize());

            chunks.push(ChunkInfo {
                hash,
                size: (end - offset) as u64,
                index,
            });

            offset = end;
            index += 1;
        }

        ChunkManifest {
            content_hash,
            total_size: data.len() as u64,
            chunk_size,
            chunks,
        }
    }

    /// Create a chunk manifest event (kind 9000).
    ///
    /// Content = JSON manifest. D-tag = content_hash (parameterized
    /// replaceable -- latest manifest for the same content from the
    /// same author wins).
    pub fn manifest(
        manifest: &ChunkManifest,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        if manifest.chunks.is_empty() {
            return Err(GlobeError::InvalidConfig(
                "manifest has no chunks".into(),
            ));
        }
        if manifest.content_hash.len() != 64
            || !manifest.content_hash.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(GlobeError::InvalidConfig(
                "content_hash must be 64 hex characters".into(),
            ));
        }

        let content = serde_json::to_string(manifest)
            .map_err(|e| GlobeError::InvalidConfig(format!("manifest serialization: {e}")))?;

        let unsigned = UnsignedEvent::new(kind::CHUNK_MANIFEST, &content)
            .with_d_tag(&manifest.content_hash)
            .with_tag("chunks", &[&manifest.chunks.len().to_string()])
            .with_tag("size", &[&manifest.total_size.to_string()]);

        EventBuilder::sign(&unsigned, keypair)
    }

    /// Parse a chunk manifest from an event.
    pub fn parse_manifest(event: &OmniEvent) -> Result<ChunkManifest, GlobeError> {
        if event.kind != kind::CHUNK_MANIFEST {
            return Err(GlobeError::InvalidMessage(format!(
                "expected kind {}, got {}",
                kind::CHUNK_MANIFEST,
                event.kind,
            )));
        }

        serde_json::from_str(&event.content)
            .map_err(|e| GlobeError::InvalidMessage(format!("invalid manifest JSON: {e}")))
    }

    /// Verify that reassembled data matches a manifest.
    ///
    /// Checks total size, overall SHA-256, and each chunk's individual hash
    /// at the manifest's chunk boundaries.
    pub fn verify(data: &[u8], manifest: &ChunkManifest) -> bool {
        if data.len() as u64 != manifest.total_size {
            return false;
        }

        // Overall content hash.
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        if hash != manifest.content_hash {
            return false;
        }

        // Per-chunk hashes.
        let mut offset = 0usize;
        for chunk in &manifest.chunks {
            let end = offset + chunk.size as usize;
            if end > data.len() {
                return false;
            }
            let mut chunk_hasher = Sha256::new();
            chunk_hasher.update(&data[offset..end]);
            let chunk_hash = hex::encode(chunk_hasher.finalize());
            if chunk_hash != chunk.hash {
                return false;
            }
            offset = end;
        }

        offset == data.len()
    }

    /// Which chunks from a manifest are not yet available locally.
    ///
    /// Pass in the set of hashes you already have; returns references to
    /// the `ChunkInfo` entries you still need to fetch.
    pub fn missing_chunks<'a>(
        manifest: &'a ChunkManifest,
        available: &HashSet<String>,
    ) -> Vec<&'a ChunkInfo> {
        manifest
            .chunks
            .iter()
            .filter(|c| !available.contains(&c.hash))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    #[test]
    fn split_basic() {
        let data = vec![0u8; 3_000_000];
        let manifest = ChunkBuilder::split(&data, 1_048_576);
        assert_eq!(manifest.chunks.len(), 3);
        assert_eq!(manifest.total_size, 3_000_000);
        assert_eq!(manifest.chunks[0].index, 0);
        assert_eq!(manifest.chunks[1].index, 1);
        assert_eq!(manifest.chunks[2].index, 2);
        assert_eq!(manifest.chunks[0].size, 1_048_576);
        assert_eq!(manifest.chunks[1].size, 1_048_576);
        assert_eq!(manifest.chunks[2].size, 3_000_000 - 2 * 1_048_576);
    }

    #[test]
    fn split_exact_boundary() {
        let data = vec![0u8; 2_097_152]; // exactly 2 MB
        let manifest = ChunkBuilder::split(&data, 1_048_576);
        assert_eq!(manifest.chunks.len(), 2);
        assert_eq!(manifest.chunks[0].size, 1_048_576);
        assert_eq!(manifest.chunks[1].size, 1_048_576);
    }

    #[test]
    fn split_small_file() {
        let data = vec![42u8; 100];
        let manifest = ChunkBuilder::split(&data, 1_048_576);
        assert_eq!(manifest.chunks.len(), 1);
        assert_eq!(manifest.chunks[0].size, 100);
        assert_eq!(manifest.total_size, 100);
    }

    #[test]
    fn split_zero_chunk_size_uses_default() {
        let data = vec![0u8; 100];
        let manifest = ChunkBuilder::split(&data, 0);
        assert_eq!(manifest.chunk_size, DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn split_produces_unique_hashes_for_different_data() {
        let data1 = vec![0u8; 2_000_000];
        let data2 = vec![1u8; 2_000_000];
        let m1 = ChunkBuilder::split(&data1, 1_048_576);
        let m2 = ChunkBuilder::split(&data2, 1_048_576);
        assert_ne!(m1.content_hash, m2.content_hash);
        assert_ne!(m1.chunks[0].hash, m2.chunks[0].hash);
    }

    #[test]
    fn verify_correct_data() {
        let data = b"Hello, Omnidea! This is a test of chunked video files.".to_vec();
        let manifest = ChunkBuilder::split(&data, 20);
        assert!(ChunkBuilder::verify(&data, &manifest));
    }

    #[test]
    fn verify_wrong_data() {
        let data = b"correct".to_vec();
        let manifest = ChunkBuilder::split(&data, 4);
        let wrong = b"wronggg".to_vec();
        assert!(!ChunkBuilder::verify(&wrong, &manifest));
    }

    #[test]
    fn verify_wrong_size() {
        let data = b"original".to_vec();
        let manifest = ChunkBuilder::split(&data, 4);
        let shorter = b"short".to_vec();
        assert!(!ChunkBuilder::verify(&shorter, &manifest));
    }

    #[test]
    fn manifest_event_round_trip() {
        let kp = test_keypair();
        let data = vec![7u8; 500_000];
        let manifest = ChunkBuilder::split(&data, 100_000);

        let event = ChunkBuilder::manifest(&manifest, &kp).unwrap();
        assert_eq!(event.kind, kind::CHUNK_MANIFEST);
        assert_eq!(event.d_tag(), Some(manifest.content_hash.as_str()));

        let parsed = ChunkBuilder::parse_manifest(&event).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn manifest_has_metadata_tags() {
        let kp = test_keypair();
        let data = vec![0u8; 300_000];
        let manifest = ChunkBuilder::split(&data, 100_000);

        let event = ChunkBuilder::manifest(&manifest, &kp).unwrap();
        let chunks_tag = event.tag_values("chunks");
        assert_eq!(chunks_tag, vec!["3"]);
        let size_tag = event.tag_values("size");
        assert_eq!(size_tag, vec!["300000"]);
    }

    #[test]
    fn manifest_valid_signature() {
        let kp = test_keypair();
        let data = vec![0u8; 100];
        let manifest = ChunkBuilder::split(&data, 50);
        let event = ChunkBuilder::manifest(&manifest, &kp).unwrap();
        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn manifest_empty_chunks_rejected() {
        let kp = test_keypair();
        let manifest = ChunkManifest {
            content_hash: "a".repeat(64),
            total_size: 0,
            chunk_size: 1_048_576,
            chunks: vec![],
        };
        assert!(ChunkBuilder::manifest(&manifest, &kp).is_err());
    }

    #[test]
    fn manifest_invalid_hash_rejected() {
        let kp = test_keypair();
        let manifest = ChunkManifest {
            content_hash: "tooshort".into(),
            total_size: 100,
            chunk_size: 100,
            chunks: vec![ChunkInfo {
                hash: "a".repeat(64),
                size: 100,
                index: 0,
            }],
        };
        assert!(ChunkBuilder::manifest(&manifest, &kp).is_err());
    }

    #[test]
    fn parse_wrong_kind_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "{}".into(),
            sig: "c".repeat(128),
        };
        assert!(ChunkBuilder::parse_manifest(&event).is_err());
    }

    #[test]
    fn missing_chunks_basic() {
        // Use varied data so each chunk hashes differently.
        let mut data = vec![0u8; 300];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        let manifest = ChunkBuilder::split(&data, 100);
        assert_eq!(manifest.chunks.len(), 3);

        let mut available = HashSet::new();
        available.insert(manifest.chunks[0].hash.clone());

        let missing = ChunkBuilder::missing_chunks(&manifest, &available);
        assert_eq!(missing.len(), 2);
        assert_eq!(missing[0].index, 1);
        assert_eq!(missing[1].index, 2);
    }

    #[test]
    fn missing_chunks_all_available() {
        let mut data = vec![0u8; 200];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        let manifest = ChunkBuilder::split(&data, 100);
        let available: HashSet<String> =
            manifest.chunks.iter().map(|c| c.hash.clone()).collect();
        let missing = ChunkBuilder::missing_chunks(&manifest, &available);
        assert!(missing.is_empty());
    }

    #[test]
    fn manifest_serde_round_trip() {
        let manifest = ChunkManifest {
            content_hash: "a".repeat(64),
            total_size: 3_000_000,
            chunk_size: 1_048_576,
            chunks: vec![
                ChunkInfo { hash: "b".repeat(64), size: 1_048_576, index: 0 },
                ChunkInfo { hash: "c".repeat(64), size: 1_048_576, index: 1 },
                ChunkInfo { hash: "d".repeat(64), size: 902_848, index: 2 },
            ],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let loaded: ChunkManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, loaded);
    }

    #[test]
    fn split_deterministic() {
        let data = vec![99u8; 500];
        let m1 = ChunkBuilder::split(&data, 200);
        let m2 = ChunkBuilder::split(&data, 200);
        assert_eq!(m1, m2);
    }
}
