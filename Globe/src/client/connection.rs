use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use crate::config::GlobeConfig;
use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::filter::OmniFilter;
use crate::health::{ConnectionState, RelayHealth};
use crate::protocol::{ClientMessage, RelayMessage};

/// An event or control message received from a relay.
///
/// Delivered via the broadcast channel on [`RelayHandle`]. Subscribe with
/// `handle.subscribe_events()` to receive these in your application.
#[derive(Clone, Debug)]
pub enum RelayEvent {
    /// An event matching a subscription.
    Event {
        /// Which subscription this event matched.
        subscription_id: String,
        /// The matched event.
        event: OmniEvent,
        /// Which relay delivered it.
        source_relay: Url,
    },
    /// End of stored events — live events follow.
    StoredComplete {
        /// Which subscription finished its stored-event replay.
        subscription_id: String,
        /// Which relay sent the STORED marker.
        source_relay: Url,
    },
}

/// A handle to a relay connection task.
///
/// This is the external API — it's `Send + Sync` and communicates
/// with the background connection task via channels.
#[derive(Clone)]
pub struct RelayHandle {
    command_tx: mpsc::Sender<Command>,
    state_rx: watch::Receiver<ConnectionState>,
    event_tx: broadcast::Sender<RelayEvent>,
    url: Url,
    health: Arc<Mutex<RelayHealth>>,
}

/// Commands sent to the connection task.
pub(crate) enum Command {
    Publish {
        event: OmniEvent,
        response: oneshot::Sender<Result<(), GlobeError>>,
    },
    Subscribe {
        id: String,
        filters: Vec<OmniFilter>,
    },
    Unsubscribe(String),
    Disconnect,
}

impl RelayHandle {
    /// Create a new relay handle and spawn its connection task.
    ///
    /// The task connects to the relay via WebSocket, processes commands,
    /// and reconnects with exponential backoff on disconnection.
    pub fn new(url: Url, config: Arc<GlobeConfig>) -> Self {
        let (command_tx, command_rx) = mpsc::channel(256);
        let (state_tx, state_rx) = watch::channel(ConnectionState::Disconnected);
        let (event_tx, _event_rx) = broadcast::channel(1024);
        let health = Arc::new(Mutex::new(RelayHealth::new(url.clone())));

        let task_url = url.clone();
        let task_health = health.clone();
        let task_event_tx = event_tx.clone();
        tokio::spawn(connection_task(
            task_url,
            config,
            command_rx,
            state_tx,
            task_event_tx,
            task_health,
        ));

        Self {
            command_tx,
            state_rx,
            event_tx,
            url,
            health,
        }
    }

    /// The relay URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state_rx.borrow().clone()
    }

    /// Subscribe to events from this relay.
    pub fn subscribe_events(&self) -> broadcast::Receiver<RelayEvent> {
        self.event_tx.subscribe()
    }

    /// Get a snapshot of the relay's health metrics.
    pub fn health(&self) -> RelayHealth {
        self.health.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Publish an event to this relay.
    pub async fn publish(&self, event: OmniEvent) -> Result<(), GlobeError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(Command::Publish {
                event,
                response: tx,
            })
            .await
            .map_err(|_| GlobeError::ConnectionClosed {
                url: self.url.clone(),
                reason: Some("command channel closed".into()),
            })?;
        rx.await.map_err(|_| GlobeError::ConnectionClosed {
            url: self.url.clone(),
            reason: Some("response channel closed".into()),
        })?
    }

    /// Subscribe to events matching the given filters.
    pub async fn subscribe(&self, id: String, filters: Vec<OmniFilter>) -> Result<(), GlobeError> {
        self.command_tx
            .send(Command::Subscribe { id, filters })
            .await
            .map_err(|_| GlobeError::ConnectionClosed {
                url: self.url.clone(),
                reason: Some("command channel closed".into()),
            })
    }

    /// Unsubscribe from a subscription.
    pub async fn unsubscribe(&self, id: String) -> Result<(), GlobeError> {
        self.command_tx
            .send(Command::Unsubscribe(id))
            .await
            .map_err(|_| GlobeError::ConnectionClosed {
                url: self.url.clone(),
                reason: Some("command channel closed".into()),
            })
    }

    /// Disconnect from the relay.
    pub async fn disconnect(&self) -> Result<(), GlobeError> {
        let _ = self.command_tx.send(Command::Disconnect).await;
        Ok(())
    }

    /// Watch for connection state changes.
    pub fn watch_state(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }
}

// ---------------------------------------------------------------------------
// Connection task
// ---------------------------------------------------------------------------

/// The background connection task.
///
/// Connects to the relay via WebSocket, processes commands, forwards
/// incoming events, and reconnects with exponential backoff.
async fn connection_task(
    url: Url,
    config: Arc<GlobeConfig>,
    mut command_rx: mpsc::Receiver<Command>,
    state_tx: watch::Sender<ConnectionState>,
    event_tx: broadcast::Sender<RelayEvent>,
    health: Arc<Mutex<RelayHealth>>,
) {
    let mut subscriptions: HashMap<String, Vec<OmniFilter>> = HashMap::new();
    let mut attempt: u32 = 0;
    let mut disconnect_requested = false;

    log::debug!("connection task started for {url}");

    loop {
        if disconnect_requested {
            let _ = state_tx.send(ConnectionState::Disconnected);
            break;
        }

        // --- Phase 1: Connect (while processing commands) ---

        let _ = state_tx.send(ConnectionState::Connecting);

        let connect_future = tokio::time::timeout(
            config.connection_timeout,
            tokio_tungstenite::connect_async(url.as_str()),
        );
        tokio::pin!(connect_future);

        let mut connected_ws = None;

        loop {
            tokio::select! {
                biased;

                cmd = command_rx.recv() => {
                    match process_command_while_disconnected(cmd, &mut subscriptions) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => {
                            disconnect_requested = true;
                            break;
                        }
                    }
                }

                result = &mut connect_future => {
                    match result {
                        Ok(Ok((ws, _))) => {
                            connected_ws = Some(ws);
                        }
                        Ok(Err(e)) => {
                            log::warn!("connection to {url} failed: {e}");
                            health.lock().unwrap_or_else(|e| e.into_inner()).record_error();
                        }
                        Err(_) => {
                            log::warn!("connection to {url} timed out");
                            health.lock().unwrap_or_else(|e| e.into_inner()).record_error();
                        }
                    }
                    break;
                }
            }
        }

        if disconnect_requested {
            continue;
        }

        // --- Phase 2: Run connected session ---

        if let Some(ws) = connected_ws {
            attempt = 0;
            let _ = state_tx.send(ConnectionState::Connected);
            {
                let mut h = health.lock().unwrap_or_else(|e| e.into_inner());
                h.state = ConnectionState::Connected;
                h.connected_since = Some(Utc::now());
            }
            log::info!("connected to {url}");

            disconnect_requested = run_connected(
                ws,
                &url,
                &config,
                &mut command_rx,
                &event_tx,
                &health,
                &mut subscriptions,
            )
            .await;

            {
                let mut h = health.lock().unwrap_or_else(|e| e.into_inner());
                h.state = ConnectionState::Disconnected;
                h.connected_since = None;
            }

            if disconnect_requested {
                continue;
            }
        }

        // --- Phase 3: Backoff before reconnecting ---

        attempt += 1;
        if let Some(max) = config.reconnect_max_attempts {
            if attempt >= max {
                let _ = state_tx.send(ConnectionState::Failed {
                    reason: format!("max reconnection attempts ({max}) reached"),
                });
                break;
            }
        }

        let delay = backoff_delay(attempt, &config);
        let _ = state_tx.send(ConnectionState::Reconnecting { attempt });
        log::debug!("reconnecting to {url} in {delay:?} (attempt {attempt})");

        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                biased;

                cmd = command_rx.recv() => {
                    match process_command_while_disconnected(cmd, &mut subscriptions) {
                        ControlFlow::Continue(()) => continue,
                        ControlFlow::Break(()) => {
                            disconnect_requested = true;
                            break;
                        }
                    }
                }

                () = &mut sleep => break,
            }
        }
    }

    log::debug!("connection task ended for {url}");
}

/// Run a connected WebSocket session.
///
/// Returns `true` if disconnect was explicitly requested, `false` if
/// the connection was lost and reconnection should be attempted.
async fn run_connected(
    ws: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    url: &Url,
    config: &GlobeConfig,
    command_rx: &mut mpsc::Receiver<Command>,
    event_tx: &broadcast::Sender<RelayEvent>,
    health: &Arc<Mutex<RelayHealth>>,
    subscriptions: &mut HashMap<String, Vec<OmniFilter>>,
) -> bool {
    let (mut write, mut read) = ws.split();
    let mut heartbeat = tokio::time::interval(config.heartbeat_interval);
    heartbeat.tick().await; // Skip the immediate first tick.

    // Pending publish OK responses, keyed by event_id.
    let mut pending_oks: HashMap<String, oneshot::Sender<Result<(), GlobeError>>> = HashMap::new();

    // Resubscribe existing subscriptions.
    for (sub_id, filters) in subscriptions.iter() {
        let msg = ClientMessage::Req {
            subscription_id: sub_id.clone(),
            filters: filters.clone(),
        };
        if let Ok(json) = msg.to_json() {
            if write.send(Message::Text(json.into())).await.is_err() {
                return false;
            }
        }
    }

    loop {
        tokio::select! {
            // Command from RelayHandle.
            cmd = command_rx.recv() => {
                let cmd = match cmd {
                    Some(c) => c,
                    None => return false,
                };

                match cmd {
                    Command::Publish { event, response } => {
                        let event_id = event.id.clone();
                        let msg = ClientMessage::Event(event);
                        match msg.to_json() {
                            Ok(json) => {
                                if write.send(Message::Text(json.into())).await.is_err() {
                                    let _ = response.send(Err(GlobeError::ConnectionClosed {
                                        url: url.clone(),
                                        reason: Some("write failed".into()),
                                    }));
                                    return false;
                                }
                                health.lock().unwrap_or_else(|e| e.into_inner()).record_send();
                                pending_oks.insert(event_id, response);
                            }
                            Err(e) => {
                                let _ = response.send(Err(e));
                            }
                        }
                    }

                    Command::Subscribe { id, filters } => {
                        let msg = ClientMessage::Req {
                            subscription_id: id.clone(),
                            filters: filters.clone(),
                        };
                        if let Ok(json) = msg.to_json() {
                            if write.send(Message::Text(json.into())).await.is_err() {
                                return false;
                            }
                        }
                        subscriptions.insert(id, filters);
                    }

                    Command::Unsubscribe(id) => {
                        let msg = ClientMessage::Close(id.clone());
                        if let Ok(json) = msg.to_json() {
                            let _ = write.send(Message::Text(json.into())).await;
                        }
                        subscriptions.remove(&id);
                    }

                    Command::Disconnect => {
                        let _ = write.send(Message::Close(None)).await;
                        return true;
                    }
                }
            }

            // Incoming frame from relay.
            frame = read.next() => {
                match frame {
                    Some(Ok(Message::Text(text))) => {
                        health.lock().unwrap_or_else(|e| e.into_inner()).record_receive();
                        match RelayMessage::from_json(&text) {
                            Ok(relay_msg) => {
                                handle_relay_message(
                                    relay_msg,
                                    url,
                                    event_tx,
                                    &mut pending_oks,
                                );
                            }
                            Err(e) => {
                                log::debug!("invalid relay message from {url}: {e}");
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Heartbeat acknowledged.
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        log::debug!("connection to {url} closed by relay");
                        return false;
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Binary frames — future media support.
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        log::debug!("read error from {url}: {e}");
                        health.lock().unwrap_or_else(|e| e.into_inner()).record_error();
                        return false;
                    }
                }
            }

            // Heartbeat ping.
            _ = heartbeat.tick() => {
                if write.send(Message::Ping(vec![].into())).await.is_err() {
                    log::debug!("heartbeat ping to {url} failed");
                    return false;
                }
            }
        }
    }
}

/// Route a parsed relay message to the appropriate handler.
fn handle_relay_message(
    msg: RelayMessage,
    url: &Url,
    event_tx: &broadcast::Sender<RelayEvent>,
    pending_oks: &mut HashMap<String, oneshot::Sender<Result<(), GlobeError>>>,
) {
    match msg {
        RelayMessage::Event {
            subscription_id,
            event,
        } => {
            let _ = event_tx.send(RelayEvent::Event {
                subscription_id,
                event,
                source_relay: url.clone(),
            });
        }
        RelayMessage::Stored(sub_id) => {
            let _ = event_tx.send(RelayEvent::StoredComplete {
                subscription_id: sub_id,
                source_relay: url.clone(),
            });
        }
        RelayMessage::Ok {
            event_id,
            success,
            message,
        } => {
            if let Some(sender) = pending_oks.remove(&event_id) {
                if success {
                    let _ = sender.send(Ok(()));
                } else {
                    let _ = sender.send(Err(GlobeError::EventRejected {
                        event_id,
                        reason: message.unwrap_or_default(),
                    }));
                }
            }
        }
        RelayMessage::Notice(message) => {
            log::info!("relay notice from {url}: {message}");
        }
        RelayMessage::Closed {
            subscription_id,
            reason,
        } => {
            log::debug!("subscription {subscription_id} closed by {url}: {reason:?}");
        }
        RelayMessage::Auth(challenge) => {
            log::debug!("auth challenge from {url}: {challenge}");
        }
        RelayMessage::SearchResult {
            subscription_id,
            event,
            ..
        } => {
            // Forward search results as regular events to the client.
            let _ = event_tx.send(RelayEvent::Event {
                subscription_id,
                event,
                source_relay: url.clone(),
            });
        }
    }
}

/// Process a command while not connected (during connect attempt or backoff).
///
/// Publishes are immediately failed with `NotConnected`. Subscriptions are
/// accumulated so they can be sent when the connection is established.
fn process_command_while_disconnected(
    cmd: Option<Command>,
    subscriptions: &mut HashMap<String, Vec<OmniFilter>>,
) -> ControlFlow<()> {
    match cmd {
        Some(Command::Disconnect) | None => ControlFlow::Break(()),
        Some(Command::Subscribe { id, filters }) => {
            subscriptions.insert(id, filters);
            ControlFlow::Continue(())
        }
        Some(Command::Unsubscribe(id)) => {
            subscriptions.remove(&id);
            ControlFlow::Continue(())
        }
        Some(Command::Publish { response, .. }) => {
            let _ = response.send(Err(GlobeError::NotConnected));
            ControlFlow::Continue(())
        }
    }
}

/// Calculate exponential backoff delay.
fn backoff_delay(attempt: u32, config: &GlobeConfig) -> Duration {
    let base = config.reconnect_min_delay;
    let max = config.reconnect_max_delay;
    let multiplier = 2u32.saturating_pow(attempt.saturating_sub(1));
    let delay = base.saturating_mul(multiplier);
    delay.min(max)
}

impl std::fmt::Debug for RelayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayHandle")
            .field("url", &self.url)
            .field("state", &self.state())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Arc<GlobeConfig> {
        Arc::new(GlobeConfig::default())
    }

    #[tokio::test]
    async fn new_handle_starts_disconnected() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url.clone(), test_config());
        // State is Disconnected before the spawned task runs.
        assert_eq!(handle.state(), ConnectionState::Disconnected);
        assert_eq!(handle.url(), &url);
    }

    #[tokio::test]
    async fn health_starts_clean() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());
        let health = handle.health();
        assert_eq!(health.send_count, 0);
        assert_eq!(health.receive_count, 0);
        assert_eq!(health.error_count, 0);
    }

    #[tokio::test]
    async fn subscribe_events_returns_receiver() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());
        let _rx = handle.subscribe_events();
    }

    #[tokio::test]
    async fn debug_format() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());
        let debug = format!("{handle:?}");
        assert!(debug.contains("relay.example.com"));
    }

    #[tokio::test]
    async fn watch_state_clones() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());
        let rx = handle.watch_state();
        assert_eq!(*rx.borrow(), ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn publish_fails_when_not_connected() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());

        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 0,
            kind: 1,
            tags: vec![],
            content: "test".into(),
            sig: "c".repeat(128),
        };

        // Task is connecting to a non-existent relay.
        // Publish should fail with NotConnected.
        let result = handle.publish(event).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn subscribe_reaches_task() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());
        let result = handle.subscribe("sub-1".into(), vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn disconnect_stops_task() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let handle = RelayHandle::new(url, test_config());

        let result = handle.disconnect().await;
        assert!(result.is_ok());

        // Give the task a moment to shut down.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // After disconnect, further commands should fail (channel closed).
        let result = handle.subscribe("sub-2".into(), vec![]).await;
        assert!(result.is_err());
    }

    #[test]
    fn backoff_delay_exponential() {
        let config = GlobeConfig::default();
        let d1 = backoff_delay(1, &config);
        let d2 = backoff_delay(2, &config);
        let d3 = backoff_delay(3, &config);

        assert_eq!(d1, config.reconnect_min_delay);
        assert_eq!(d2, config.reconnect_min_delay * 2);
        assert_eq!(d3, config.reconnect_min_delay * 4);
    }

    #[test]
    fn backoff_delay_caps_at_max() {
        let config = GlobeConfig::default();
        let d20 = backoff_delay(20, &config);
        assert_eq!(d20, config.reconnect_max_delay);
    }
}
