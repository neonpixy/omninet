use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::event::OmniEvent;
use crate::filter::OmniFilter;
use crate::protocol::{ClientMessage, RelayMessage};

use super::storage::EventStore;

/// A binary frame broadcast message.
///
/// Binary frames carry real-time data (audio, video) tagged with a
/// Communicator session ID. The `sender` field prevents the session that
/// sent the frame from receiving its own echo.
#[derive(Clone, Debug)]
pub struct BinaryBroadcast {
    /// Unique ID of the sending WebSocket session (for echo suppression).
    pub sender: u64,
    /// The raw binary frame (session_id_len + session_id + payload).
    pub data: Vec<u8>,
}

/// Handle a single client session on the relay server.
///
/// Processes one WebSocket connection end-to-end: parses client messages,
/// queries the store, manages subscriptions, forwards live events,
/// and routes binary frames for real-time communication.
#[allow(clippy::too_many_arguments)]
pub async fn handle_session<S: AsyncRead + AsyncWrite + Unpin>(
    ws: WebSocketStream<S>,
    store: EventStore,
    live_tx: broadcast::Sender<OmniEvent>,
    binary_tx: broadcast::Sender<BinaryBroadcast>,
    session_id: u64,
    event_filter: Option<super::listener::EventFilter>,
    search_handler: Option<super::listener::SearchHandler>,
    require_auth: bool,
) {
    let (mut write, mut read) = ws.split();
    let mut live_rx = live_tx.subscribe();
    let mut binary_rx = binary_tx.subscribe();
    let mut subscriptions: HashMap<String, Vec<OmniFilter>> = HashMap::new();
    let mut authenticated_author: Option<String> = None;

    loop {
        tokio::select! {
            // Incoming message from the client.
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Binary(data))) => {
                        // Binary frame: forward to all other sessions.
                        // Format: [session_id_len: u8] [session_id] [payload]
                        if data.len() >= 2 {
                            let _ = binary_tx.send(BinaryBroadcast {
                                sender: session_id,
                                data: data.into(),
                            });
                        }
                        continue;
                    }
                    Some(Ok(_)) => continue,  // Ping/pong handled by tungstenite.
                    Some(Err(e)) => {
                        log::debug!("session read error: {e}");
                        break;
                    }
                };

                let client_msg = match ClientMessage::from_json(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        let notice = RelayMessage::Notice(format!("invalid message: {e}"));
                        let _ = send_relay_msg(&mut write, &notice).await;
                        continue;
                    }
                };

                match client_msg {
                    ClientMessage::Event(event) => {
                        if require_auth && authenticated_author.is_none() {
                            let _ = send_relay_msg(
                                &mut write,
                                &RelayMessage::Notice("authentication required".into()),
                            ).await;
                            continue;
                        }

                        let event_id = event.id.clone();

                        // Check event filter (Tower content policy, etc.).
                        if let Some(ref filter) = event_filter {
                            if !filter(&event) {
                                let reply = RelayMessage::Ok {
                                    event_id,
                                    success: false,
                                    message: Some("blocked: event kind not accepted".into()),
                                };
                                let _ = send_relay_msg(&mut write, &reply).await;
                                continue;
                            }
                        }

                        let is_new = store.insert(event.clone());
                        let reply = RelayMessage::Ok {
                            event_id,
                            success: true,
                            message: if is_new {
                                None
                            } else {
                                Some("duplicate".into())
                            },
                        };
                        let _ = send_relay_msg(&mut write, &reply).await;

                        // Broadcast to other sessions for live delivery.
                        if is_new {
                            let _ = live_tx.send(event);
                        }
                    }

                    ClientMessage::Req { subscription_id, filters } => {
                        if require_auth && authenticated_author.is_none() {
                            let _ = send_relay_msg(
                                &mut write,
                                &RelayMessage::Notice("authentication required".into()),
                            ).await;
                            continue;
                        }

                        // Check if any filter has a search query and we have a handler.
                        let has_search = filters.iter().any(|f| f.search.is_some());

                        if has_search {
                            if let Some(ref handler) = search_handler {
                                // Delegate search filters to the search handler.
                                for filter in &filters {
                                    if let Some(ref query) = filter.search {
                                        let hits = handler(query, filter);
                                        for hit in &hits {
                                            // Look up the full event from the store.
                                            if let Some(event) = store.get(&hit.event_id) {
                                                let msg = RelayMessage::SearchResult {
                                                    subscription_id: subscription_id.clone(),
                                                    event,
                                                    relevance: hit.relevance,
                                                    snippet: hit.snippet.clone(),
                                                    suggestions: hit.suggestions.clone(),
                                                };
                                                if send_relay_msg(&mut write, &msg).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Also run normal stored-event query for all filters.
                        let stored = store.query_any(&filters);
                        for event in stored {
                            let msg = RelayMessage::Event {
                                subscription_id: subscription_id.clone(),
                                event,
                            };
                            if send_relay_msg(&mut write, &msg).await.is_err() {
                                break;
                            }
                        }

                        // Signal end of stored events.
                        let _ = send_relay_msg(
                            &mut write,
                            &RelayMessage::Stored(subscription_id.clone()),
                        ).await;

                        // Register for live events.
                        subscriptions.insert(subscription_id, filters);
                    }

                    ClientMessage::Close(sub_id) => {
                        subscriptions.remove(&sub_id);
                    }

                    ClientMessage::Auth(event) => {
                        // Verify the AUTH event signature and kind.
                        if event.kind == 22242 {
                            use crate::event_builder::EventBuilder;
                            match EventBuilder::verify(&event) {
                                Ok(true) => {
                                    authenticated_author = Some(event.author.clone());
                                    let _ = send_relay_msg(
                                        &mut write,
                                        &RelayMessage::Ok {
                                            event_id: event.id.clone(),
                                            success: true,
                                            message: Some("authenticated".into()),
                                        },
                                    ).await;
                                }
                                _ => {
                                    let _ = send_relay_msg(
                                        &mut write,
                                        &RelayMessage::Ok {
                                            event_id: event.id.clone(),
                                            success: false,
                                            message: Some("invalid signature".into()),
                                        },
                                    ).await;
                                }
                            }
                        } else {
                            let _ = send_relay_msg(
                                &mut write,
                                &RelayMessage::Notice("auth requires kind 22242".into()),
                            ).await;
                        }
                    }
                }
            }

            // Live event from another session.
            live_event = live_rx.recv() => {
                let event = match live_event {
                    Ok(e) => e,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("session lagged {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                // Check if any subscription matches this event.
                for (sub_id, filters) in &subscriptions {
                    if filters.iter().any(|f| f.matches(&event)) {
                        let msg = RelayMessage::Event {
                            subscription_id: sub_id.clone(),
                            event: event.clone(),
                        };
                        if send_relay_msg(&mut write, &msg).await.is_err() {
                            return; // Connection lost.
                        }
                    }
                }
            }

            // Binary frame from another session.
            binary = binary_rx.recv() => {
                match binary {
                    Ok(frame) if frame.sender != session_id => {
                        // Forward to this client.
                        if write.send(Message::Binary(frame.data.into())).await.is_err() {
                            return; // Connection lost.
                        }
                    }
                    Ok(_) => {} // Skip our own frames (echo suppression).
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("session {session_id} lagged {n} binary frames");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    log::debug!("session ended");
}

/// Send a RelayMessage over the WebSocket.
async fn send_relay_msg<S>(
    write: &mut S,
    msg: &RelayMessage,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = msg.to_json()?;
    write.send(Message::Text(json.into())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Session tests require a WebSocket pair (client + server).
    // These are tested via the integration test that starts a full relay.

    #[test]
    fn subscription_map_works() {
        let mut subs: HashMap<String, Vec<OmniFilter>> = HashMap::new();
        subs.insert(
            "sub-1".into(),
            vec![OmniFilter {
                kinds: Some(vec![1]),
                ..Default::default()
            }],
        );
        assert_eq!(subs.len(), 1);
        subs.remove("sub-1");
        assert!(subs.is_empty());
    }
}
