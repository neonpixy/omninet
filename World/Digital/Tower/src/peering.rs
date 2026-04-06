use std::sync::Arc;
use std::time::Duration;

use globe::config::GlobeConfig;
use globe::gospel::peer::GospelPeer;
use globe::gospel::registry::GospelRegistry;
use globe::gospel::tier::GospelTier;
use url::Url;

/// Manages gospel peering with seed peers and discovered Tower nodes.
///
/// Runs a periodic evangelization loop: connect to seed peers,
/// exchange gospel records, discover new Tower nodes from lighthouse
/// announcements, and add them to the peer list.
///
/// Supports two sync modes:
/// - **Bilateral sync** (`evangelize_all`) — full catch-up on a timer.
/// - **Live sync** (`recv_live_all`) — drain persistent subscriptions.
pub struct PeeringLoop {
    peers: Vec<GospelPeer>,
    globe_config: Arc<GlobeConfig>,
    interval: Duration,
    max_peers: usize,
    tiers: Vec<GospelTier>,
}

impl PeeringLoop {
    /// Create a new peering loop with seed peer URLs and tier config.
    pub fn new(
        seed_urls: &[Url],
        interval: Duration,
        max_peers: usize,
        tiers: Vec<GospelTier>,
    ) -> Self {
        let globe_config = Arc::new(GlobeConfig::default());
        let peers: Vec<GospelPeer> = seed_urls
            .iter()
            .take(max_peers)
            .map(|url| GospelPeer::new(url.clone(), globe_config.clone(), tiers.clone()))
            .collect();

        Self {
            peers,
            globe_config,
            interval,
            max_peers,
            tiers,
        }
    }

    /// The configured evangelization interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Number of currently configured peers.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Which gospel tiers this loop propagates.
    pub fn tiers(&self) -> &[GospelTier] {
        &self.tiers
    }

    /// Add a peer URL if not already present and under the max.
    /// Returns true if the peer was added.
    pub fn add_peer(&mut self, url: Url) -> bool {
        if self.peers.len() >= self.max_peers {
            return false;
        }
        if self.peers.iter().any(|p| *p.url() == url) {
            return false;
        }
        self.peers.push(GospelPeer::new(
            url,
            self.globe_config.clone(),
            self.tiers.clone(),
        ));
        true
    }

    /// Run one evangelization cycle with all peers.
    ///
    /// Returns (total_received, total_sent) across all peers.
    pub async fn evangelize_all(
        &mut self,
        registry: &GospelRegistry,
    ) -> (usize, usize) {
        let mut total_received = 0;
        let mut total_sent = 0;

        for peer in &mut self.peers {
            match peer.evangelize(registry).await {
                Ok((received, sent)) => {
                    total_received += received;
                    total_sent += sent;
                    if received > 0 || sent > 0 {
                        log::info!(
                            "gospel sync with {}: received={received}, sent={sent}",
                            peer.url()
                        );
                    }
                }
                Err(e) => {
                    log::warn!("gospel sync with {} failed: {e}", peer.url());
                }
            }
        }

        (total_received, total_sent)
    }

    /// Drain live events from all peers' persistent subscriptions.
    ///
    /// Non-blocking. Returns total new events merged into the registry.
    pub fn recv_live_all(&mut self, registry: &GospelRegistry) -> usize {
        let mut total = 0;
        for peer in &mut self.peers {
            total += peer.recv_live(registry);
        }
        total
    }

    /// Open live subscriptions on all peers that don't have one yet.
    ///
    /// Must be called within an async context (needs await for subscribe).
    pub async fn open_live_subscriptions(&mut self) {
        for peer in &mut self.peers {
            if !peer.has_live_subscription() {
                if let Err(e) = peer.open_live_subscription().await {
                    log::warn!(
                        "gospel: failed to open live subscription to {}: {e}",
                        peer.url()
                    );
                }
            }
        }
    }

    /// Take all peers out for async work (returns them, leaves self empty).
    /// Call `restore_peers()` when done to put them back.
    pub fn take_peers(&mut self) -> Vec<GospelPeer> {
        std::mem::take(&mut self.peers)
    }

    /// Restore peers after async work.
    pub fn restore_peers(&mut self, peers: Vec<GospelPeer>) {
        self.peers = peers;
    }

    /// Get peer URLs for status reporting.
    pub fn peer_urls(&self) -> Vec<String> {
        self.peers.iter().map(|p| p.url().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_peering_loop() {
        let seeds = vec![
            Url::parse("wss://seed1.example.com").unwrap(),
            Url::parse("wss://seed2.example.com").unwrap(),
        ];
        let peering = PeeringLoop::new(
            &seeds,
            Duration::from_secs(60),
            16,
            GospelTier::all(),
        );
        assert_eq!(peering.peer_count(), 2);
        assert_eq!(peering.interval(), Duration::from_secs(60));
        assert_eq!(peering.tiers().len(), 3);
    }

    #[tokio::test]
    async fn new_with_universal_only() {
        let peering = PeeringLoop::new(
            &[],
            Duration::from_secs(60),
            16,
            vec![GospelTier::Universal],
        );
        assert_eq!(peering.tiers().len(), 1);
        assert_eq!(peering.tiers()[0], GospelTier::Universal);
    }

    #[tokio::test]
    async fn add_peer() {
        let mut peering = PeeringLoop::new(
            &[],
            Duration::from_secs(60),
            3,
            GospelTier::all(),
        );
        assert_eq!(peering.peer_count(), 0);

        let url = Url::parse("wss://new-peer.example.com").unwrap();
        assert!(peering.add_peer(url.clone()));
        assert_eq!(peering.peer_count(), 1);

        // Duplicate rejected.
        assert!(!peering.add_peer(url));
        assert_eq!(peering.peer_count(), 1);
    }

    #[tokio::test]
    async fn max_peers_respected() {
        let mut peering = PeeringLoop::new(
            &[],
            Duration::from_secs(60),
            2,
            GospelTier::all(),
        );
        assert!(peering.add_peer(Url::parse("wss://a.example.com").unwrap()));
        assert!(peering.add_peer(Url::parse("wss://b.example.com").unwrap()));
        assert!(!peering.add_peer(Url::parse("wss://c.example.com").unwrap()));
        assert_eq!(peering.peer_count(), 2);
    }

    #[tokio::test]
    async fn seed_peers_capped_at_max() {
        let seeds: Vec<Url> = (0..20)
            .map(|i| Url::parse(&format!("wss://seed{i}.example.com")).unwrap())
            .collect();
        let peering = PeeringLoop::new(
            &seeds,
            Duration::from_secs(60),
            5,
            GospelTier::all(),
        );
        assert_eq!(peering.peer_count(), 5);
    }

    #[tokio::test]
    async fn take_and_restore_peers() {
        let seeds = vec![
            Url::parse("wss://a.example.com").unwrap(),
            Url::parse("wss://b.example.com").unwrap(),
        ];
        let mut peering = PeeringLoop::new(
            &seeds,
            Duration::from_secs(60),
            16,
            GospelTier::all(),
        );
        assert_eq!(peering.peer_count(), 2);

        // Take peers out.
        let peers = peering.take_peers();
        assert_eq!(peers.len(), 2);
        assert_eq!(peering.peer_count(), 0);

        // Restore peers.
        peering.restore_peers(peers);
        assert_eq!(peering.peer_count(), 2);
    }

    #[tokio::test]
    async fn peer_urls() {
        let seeds = vec![
            Url::parse("wss://a.example.com").unwrap(),
            Url::parse("wss://b.example.com").unwrap(),
        ];
        let peering = PeeringLoop::new(
            &seeds,
            Duration::from_secs(60),
            16,
            GospelTier::all(),
        );
        let urls = peering.peer_urls();
        assert_eq!(urls.len(), 2);
        assert!(urls.iter().any(|u| u.contains("a.example.com")));
    }

    #[tokio::test]
    async fn recv_live_without_subscriptions_returns_zero() {
        let mut peering = PeeringLoop::new(
            &[Url::parse("wss://a.example.com").unwrap()],
            Duration::from_secs(60),
            16,
            GospelTier::all(),
        );
        let config = globe::gospel::config::GospelConfig::default();
        let registry = globe::gospel::registry::GospelRegistry::new(&config);
        assert_eq!(peering.recv_live_all(&registry), 0);
    }
}
