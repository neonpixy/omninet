//! Pull-through caching — fetch assets from peer relays on cache miss.
//!
//! Features:
//! - **Gospel-aware peer selection**: consults the GospelRegistry for asset
//!   announcements to find which peer actually has the asset.
//! - **Fetch coalescing**: if multiple clients request the same missing asset
//!   simultaneously, only one peer fetch is made. Others wait for it.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use url::Url;

use crate::asset;
use crate::error::GlobeError;
use crate::gospel::GospelRegistry;

use super::asset_store::AssetStore;

/// Coalesces concurrent fetch requests for the same asset hash.
///
/// When multiple clients request a missing asset simultaneously, only
/// one peer fetch is made. Other requesters wait for the first fetch
/// to complete, then read from the local store.
#[derive(Clone)]
pub struct FetchCoalescer {
    /// Hashes currently being fetched. Each maps to a watch channel
    /// that signals when the fetch completes (true = success, false = failed).
    in_flight: Arc<Mutex<HashMap<String, watch::Receiver<bool>>>>,
}

impl FetchCoalescer {
    /// Create a new fetch coalescer with no in-flight requests.
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Try to fetch an asset, coalescing with any in-flight request.
    ///
    /// Returns `true` if the asset is now available in the store.
    pub async fn fetch_coalesced(
        &self,
        hash: &str,
        store: &AssetStore,
        peer_urls: &[Url],
        registry: Option<&GospelRegistry>,
    ) -> bool {
        // Check local store first.
        if store.exists(hash) {
            return true;
        }

        // Check if another task is already fetching this hash.
        // Extract the receiver while holding the lock, then drop the lock before awaiting.
        let existing_rx = {
            let in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            in_flight.get(hash).cloned()
        };

        if let Some(mut rx) = existing_rx {
            // Wait for the in-flight fetch to complete.
            let _ = rx.changed().await;
            // Check if it succeeded (asset should be in store now).
            return store.exists(hash);
        }

        // We're the first — register ourselves as in-flight.
        let (tx, rx) = watch::channel(false);
        {
            let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            in_flight.insert(hash.to_string(), rx);
        }

        // Build the list of peers to try: gospel-discovered first, then static.
        let peers = self.resolve_peers(hash, peer_urls, registry);

        // Try each peer.
        let mut success = false;
        for peer_url in &peers {
            match fetch_from_peer(hash, peer_url).await {
                Ok(data) => {
                    store.insert(hash.to_string(), data);
                    log::debug!("pull-through cached asset {hash} from {peer_url}");
                    success = true;
                    break;
                }
                Err(e) => {
                    log::debug!("peer {peer_url} failed for asset {hash}: {e}");
                }
            }
        }

        // Signal completion and clean up.
        let _ = tx.send(success);
        {
            let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            in_flight.remove(hash);
        }

        success
    }

    /// Build an ordered list of peers to try: gospel-discovered first,
    /// then static peer_urls (deduped).
    fn resolve_peers(
        &self,
        hash: &str,
        static_peers: &[Url],
        registry: Option<&GospelRegistry>,
    ) -> Vec<Url> {
        let mut peers = Vec::new();
        let mut seen = HashSet::new();

        // Gospel-aware: check registry for asset announcements.
        if let Some(reg) = registry {
            if let Some(event) = reg.lookup_asset(hash) {
                if let Ok(record) = asset::parse_announcement(&event) {
                    for url in &record.relay_urls {
                        if seen.insert(url.to_string()) {
                            peers.push(url.clone());
                        }
                    }
                }
            }
        }

        // Static peer list (fallback).
        for url in static_peers {
            if seen.insert(url.to_string()) {
                peers.push(url.clone());
            }
        }

        peers
    }
}

impl Default for FetchCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetch an asset from a peer relay's HTTP endpoint.
///
/// Connects to the peer, sends `GET /asset/{hash}`, reads the response,
/// and verifies the SHA-256 hash matches.
pub async fn fetch_from_peer(hash: &str, peer_url: &Url) -> Result<Vec<u8>, GlobeError> {
    let host = peer_url
        .host_str()
        .ok_or_else(|| GlobeError::InvalidConfig("peer URL has no host".into()))?;

    // Determine the TCP address. Use the port from the URL, defaulting
    // to 443 for wss:// and 80 for ws://.
    let default_port = if peer_url.scheme() == "wss" { 443 } else { 80 };
    let port = peer_url.port().unwrap_or(default_port);
    let addr = format!("{host}:{port}");

    let mut stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| GlobeError::ConnectionFailed {
            url: peer_url.clone(),
            reason: format!("asset fetch TCP connect failed: {e}"),
        })?;

    // Send minimal HTTP GET.
    let request = format!("GET /asset/{hash} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| GlobeError::ConnectionFailed {
            url: peer_url.clone(),
            reason: format!("asset fetch write failed: {e}"),
        })?;

    // Read entire response.
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|e| GlobeError::ConnectionFailed {
            url: peer_url.clone(),
            reason: format!("asset fetch read failed: {e}"),
        })?;

    // Parse HTTP response — find status code and body.
    let response_str = String::from_utf8_lossy(&response);

    // Extract status line.
    let status_line = response_str
        .lines()
        .next()
        .ok_or_else(|| GlobeError::InvalidMessage("empty response from peer".into()))?;

    if !status_line.contains("200") {
        return Err(GlobeError::InvalidMessage(format!(
            "peer returned {status_line} for asset {hash}"
        )));
    }

    // Find body after \r\n\r\n.
    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .or_else(|| {
            response
                .windows(2)
                .position(|w| w == b"\n\n")
                .map(|p| p + 2)
        })
        .ok_or_else(|| GlobeError::InvalidMessage("no header/body boundary in peer response".into()))?;

    let body = &response[header_end..];

    if body.is_empty() {
        return Err(GlobeError::InvalidMessage(
            "empty body from peer".into(),
        ));
    }

    // Verify SHA-256 hash.
    let computed = hex::encode(Sha256::digest(body));
    if computed != hash {
        return Err(GlobeError::InvalidMessage(format!(
            "hash mismatch from peer: expected {hash}, got {computed}"
        )));
    }

    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_peer_url_no_host() {
        let url = Url::parse("file:///tmp/no-host").unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(fetch_from_peer(&"a".repeat(64), &url));
        assert!(result.is_err());
    }

    #[test]
    fn coalescer_new() {
        let c = FetchCoalescer::new();
        assert!(c.in_flight.lock().unwrap().is_empty());
    }

    #[test]
    fn resolve_peers_static_only() {
        let c = FetchCoalescer::new();
        let statics = vec![
            Url::parse("wss://a.com").unwrap(),
            Url::parse("wss://b.com").unwrap(),
        ];
        let peers = c.resolve_peers(&"a".repeat(64), &statics, None);
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn resolve_peers_deduplicates() {
        let c = FetchCoalescer::new();
        let statics = vec![
            Url::parse("wss://a.com").unwrap(),
            Url::parse("wss://a.com").unwrap(), // duplicate
        ];
        let peers = c.resolve_peers(&"a".repeat(64), &statics, None);
        assert_eq!(peers.len(), 1);
    }
}
