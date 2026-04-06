//! Minimal HTTP handler for binary asset operations.
//!
//! Routes:
//! - `PUT /asset/{hash}` — upload asset, verify SHA-256 matches
//! - `GET /asset/{hash}` — fetch asset bytes
//! - `HEAD /asset/{hash}` — check asset existence

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

use crate::gospel::GospelRegistry;

use super::asset_fetch::FetchCoalescer;
use super::asset_store::AssetStore;

/// Check if the peeked bytes look like an HTTP asset request.
///
/// Returns `true` if the first line contains `/asset/`.
/// WebSocket upgrades go to `/` so they won't match.
pub fn is_asset_request(peeked: &[u8]) -> bool {
    let text = std::str::from_utf8(peeked).unwrap_or("");
    // Match first line only (up to \r\n or \n).
    let first_line = text.lines().next().unwrap_or("");
    first_line.contains("/asset/")
}

/// Handle an HTTP asset request on a raw TCP stream.
///
/// Reads the full HTTP request, routes to GET/PUT/HEAD, writes the response.
/// On GET cache misses, the coalescer handles smart peer discovery (via
/// gospel registry) and deduplicates concurrent fetches.
pub async fn handle_asset_request(
    mut stream: TcpStream,
    store: AssetStore,
    peer_urls: Vec<Url>,
    coalescer: FetchCoalescer,
    registry: Option<GospelRegistry>,
) {
    // Read enough to get the full request headers.
    let mut buf = vec![0u8; 8192];
    let n = match stream.read(&mut buf).await {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };

    let request_text = match std::str::from_utf8(&buf[..n]) {
        Ok(s) => s,
        Err(_) => {
            let _ = write_response(&mut stream, 400, "Bad Request", &[]).await;
            return;
        }
    };

    // Parse request line: "METHOD /path HTTP/1.1"
    let first_line = match request_text.lines().next() {
        Some(line) => line,
        None => {
            let _ = write_response(&mut stream, 400, "Bad Request", &[]).await;
            return;
        }
    };

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        let _ = write_response(&mut stream, 400, "Bad Request", &[]).await;
        return;
    }

    let method = parts[0];
    let path = parts[1];

    // Extract hash from /asset/{hash}
    let hash = match path.strip_prefix("/asset/") {
        Some(h) if !h.is_empty() => h,
        _ => {
            let _ = write_response(&mut stream, 404, "Not Found", &[]).await;
            return;
        }
    };

    // Validate hash format (64 hex chars).
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let _ = write_response(&mut stream, 400, "Invalid hash format", &[]).await;
        return;
    }

    match method {
        "GET" => handle_get(&mut stream, &store, hash, &peer_urls, &coalescer, registry.as_ref()).await,
        "HEAD" => handle_head(&mut stream, &store, hash).await,
        "PUT" => {
            // Parse Content-Length from headers.
            let content_length = parse_content_length(request_text);

            // The body may be partially in the initial read or need more reads.
            let header_end = find_header_end(request_text);
            let body_start_in_buf = header_end.min(n);
            let initial_body = &buf[body_start_in_buf..n];

            handle_put(&mut stream, &store, hash, content_length, initial_body).await;
        }
        _ => {
            let _ = write_response(&mut stream, 405, "Method Not Allowed", &[]).await;
        }
    }
}

async fn handle_get(
    stream: &mut TcpStream,
    store: &AssetStore,
    hash: &str,
    peer_urls: &[Url],
    coalescer: &FetchCoalescer,
    registry: Option<&GospelRegistry>,
) {
    // Try local store first, then coalesced peer fetch.
    if coalescer
        .fetch_coalesced(hash, store, peer_urls, registry)
        .await
    {
        if let Some(data) = store.get(hash) {
            serve_asset(stream, hash, &data).await;
            return;
        }
    }

    let _ = write_response(stream, 404, "Not Found", &[]).await;
}

async fn serve_asset(stream: &mut TcpStream, hash: &str, data: &[u8]) {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nX-Content-Hash: {}\r\n\r\n",
        data.len(),
        hash,
    );
    let _ = stream.write_all(headers.as_bytes()).await;
    let _ = stream.write_all(data).await;
}

async fn handle_head(stream: &mut TcpStream, store: &AssetStore, hash: &str) {
    if store.exists(hash) {
        // We don't know the exact size without cloning, so just report existence.
        let headers = "HTTP/1.1 200 OK\r\nX-Content-Hash: ".to_string()
            + hash
            + "\r\n\r\n";
        let _ = stream.write_all(headers.as_bytes()).await;
    } else {
        let _ = write_response(stream, 404, "Not Found", &[]).await;
    }
}

async fn handle_put(
    stream: &mut TcpStream,
    store: &AssetStore,
    hash: &str,
    content_length: Option<usize>,
    initial_body: &[u8],
) {
    let expected_len = match content_length {
        Some(len) => len,
        None => {
            let _ = write_response(stream, 411, "Length Required", &[]).await;
            return;
        }
    };

    // Read the full body.
    let mut body = Vec::with_capacity(expected_len);
    body.extend_from_slice(initial_body);

    while body.len() < expected_len {
        let mut chunk = vec![0u8; (expected_len - body.len()).min(65536)];
        match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(_) => {
                let _ = write_response(stream, 500, "Read Error", &[]).await;
                return;
            }
        }
    }

    if body.len() != expected_len {
        let _ = write_response(stream, 400, "Incomplete body", &[]).await;
        return;
    }

    // Verify SHA-256 hash matches.
    let computed = sha256_hex(&body);
    if computed != hash {
        let msg = format!("Hash mismatch: expected {hash}, got {computed}");
        let _ = write_response(stream, 409, &msg, &[]).await;
        return;
    }

    // Store it.
    if store.insert(hash.to_string(), body) {
        let _ = write_response(stream, 201, "Created", &[]).await;
    } else {
        // Duplicate or capacity exceeded — still OK for the client.
        let _ = write_response(stream, 200, "OK", &[]).await;
    }
}

/// Write a simple HTTP response.
async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &[u8],
) -> Result<(), std::io::Error> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    stream.write_all(header.as_bytes()).await?;
    if !body.is_empty() {
        stream.write_all(body).await?;
    }
    Ok(())
}

/// Parse Content-Length from raw HTTP headers.
fn parse_content_length(request: &str) -> Option<usize> {
    for line in request.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("content-length:") {
            return val.trim().parse().ok();
        }
    }
    None
}

/// Find the byte offset where headers end (after \r\n\r\n).
fn find_header_end(request: &str) -> usize {
    if let Some(pos) = request.find("\r\n\r\n") {
        pos + 4
    } else if let Some(pos) = request.find("\n\n") {
        pos + 2
    } else {
        request.len()
    }
}

/// Compute SHA-256 hash as lowercase hex.
fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_asset_request_detects_get() {
        assert!(is_asset_request(b"GET /asset/abc123 HTTP/1.1\r\n"));
    }

    #[test]
    fn is_asset_request_detects_put() {
        assert!(is_asset_request(b"PUT /asset/abc123 HTTP/1.1\r\n"));
    }

    #[test]
    fn is_asset_request_detects_head() {
        assert!(is_asset_request(b"HEAD /asset/abc123 HTTP/1.1\r\n"));
    }

    #[test]
    fn is_asset_request_rejects_websocket() {
        assert!(!is_asset_request(b"GET / HTTP/1.1\r\nUpgrade: websocket\r\n"));
    }

    #[test]
    fn is_asset_request_rejects_empty() {
        assert!(!is_asset_request(b""));
    }

    #[test]
    fn is_asset_request_rejects_garbage() {
        assert!(!is_asset_request(b"\x00\x01\x02\x03"));
    }

    #[test]
    fn parse_content_length_found() {
        let req = "PUT /asset/abc HTTP/1.1\r\nContent-Length: 1234\r\n\r\n";
        assert_eq!(parse_content_length(req), Some(1234));
    }

    #[test]
    fn parse_content_length_case_insensitive() {
        let req = "PUT /asset/abc HTTP/1.1\r\ncontent-length: 5678\r\n\r\n";
        assert_eq!(parse_content_length(req), Some(5678));
    }

    #[test]
    fn parse_content_length_missing() {
        let req = "PUT /asset/abc HTTP/1.1\r\n\r\n";
        assert_eq!(parse_content_length(req), None);
    }

    #[test]
    fn find_header_end_crlf() {
        let req = "GET /asset/x HTTP/1.1\r\nHost: localhost\r\n\r\nbody";
        assert_eq!(find_header_end(req), req.find("body").unwrap());
    }

    #[test]
    fn find_header_end_lf() {
        let req = "GET /asset/x HTTP/1.1\nHost: localhost\n\nbody";
        assert_eq!(find_header_end(req), req.find("body").unwrap());
    }

    #[test]
    fn sha256_hex_correct() {
        // SHA-256 of empty input.
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(sha256_hex(b""), expected);
    }

    #[test]
    fn sha256_hex_data() {
        let hash = sha256_hex(b"hello world");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
