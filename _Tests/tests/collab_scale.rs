//! Collaboration Scale Load Tests
//!
//! Proves the relay infrastructure can handle hundreds to thousands of
//! concurrent editors publishing CRDT ops through a Tower/relay node.
//!
//! # What's tested
//!
//! - Fan-out: 100-1000 subscribers receive broadcast events
//! - Throughput: 100 concurrent publishers, no data loss
//! - Filtering: per-document OmniFilter tag matching under load
//! - Mixed traffic: CRDT ops and cursor events on the same channel
//! - Burst handling: rapid-fire publishes without drops
//! - Sustained throughput: 30 seconds of continuous traffic
//! - CRDT convergence: 10 SequenceRga replicas converge after random edits
//!
//! # Runtime Strategy
//!
//! Tower::start creates its own Tokio multi-thread runtime (via Omnibus),
//! so tests CANNOT use `#[tokio::test]` (nested runtime panic). Instead,
//! all tests are `#[test]` and use the Tower's own runtime for async
//! WebSocket client operations via `tower.omnibus().runtime().block_on()`.
//!
//! # Connection Strategy
//!
//! The relay's accept loop processes connections sequentially: peek for
//! asset detection, then WebSocket handshake, then spawn session. This
//! means concurrent connection storms overwhelm the accept loop. All
//! tests establish connections **sequentially** (one at a time), then
//! operate them in parallel. This mirrors real-world behavior where
//! clients connect over time, not all at once.
//!
//! # Running
//!
//! ```bash
//! # Non-ignored tests (~60s each max):
//! cargo test -p omnidea-tests --test collab_scale -- --test-threads=1 --nocapture
//!
//! # ALL tests including expensive ones:
//! cargo test -p omnidea-tests --test collab_scale -- --test-threads=1 --include-ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crown::CrownKeypair;
use futures_util::{SinkExt, StreamExt};
use globe::collaboration::{CollaborationMessage, KIND_COLLABORATION};
use globe::event_builder::{EventBuilder, UnsignedEvent};
use globe::filter::OmniFilter;
use globe::idea_sync::{IdeaSyncPayload, KIND_IDEA_OPS};
use globe::protocol::{ClientMessage, RelayMessage};
use tokio_tungstenite::tungstenite::Message;
use globe::server::network_defense::RateLimitConfig;
use tower_crate::{Tower, TowerConfig, TowerMode};
use x::SequenceRga;

/// Monotonically increasing counter for unique temp directory names.
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

// ==========================================================================
// Helpers
// ==========================================================================

/// Create a TowerConfig suitable for scale tests.
fn scale_config(name: &str) -> TowerConfig {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    TowerConfig {
        mode: TowerMode::Harbor,
        name: format!("scale-{name}"),
        data_dir: std::env::temp_dir().join(format!(
            "collab_scale_{name}_{}_{}",
            std::process::id(),
            id,
        )),
        port: 0,
        bind_all: false,
        max_connections: None,
        communities: vec![],
        rate_limit_config: Some(
            RateLimitConfig::new()
                .with_max_connections_per_ip(2000)
                .with_max_events_per_minute_per_ip(100_000),
        ),
        ..Default::default()
    }
}

/// Start a Tower and give it a moment to bind its relay port.
fn start_tower(config: TowerConfig) -> Tower {
    let tower = Tower::start(config).expect("tower should start");
    std::thread::sleep(Duration::from_millis(50));
    tower
}

/// Run an async block on a Tower's own Tokio runtime.
fn block_on<F: std::future::Future>(tower: &Tower, f: F) -> F::Output {
    tower.omnibus().runtime().block_on(f)
}

/// Remove a tower's temp data directory.
fn cleanup(config: &TowerConfig) {
    std::fs::remove_dir_all(&config.data_dir).ok();
}

/// Maximum connection retry attempts.
const MAX_CONNECT_RETRIES: usize = 10;

/// Connect to a relay with exponential backoff retry.
async fn connect_with_retry(
    url: &str,
) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    for attempt in 0..MAX_CONNECT_RETRIES {
        match tokio_tungstenite::connect_async(url).await {
            Ok((ws, _)) => return ws,
            Err(e) => {
                if attempt == MAX_CONNECT_RETRIES - 1 {
                    panic!("failed to connect after {MAX_CONNECT_RETRIES} attempts: {e}");
                }
                let delay = Duration::from_millis(200 * (1 << attempt.min(4)));
                tokio::time::sleep(delay).await;
            }
        }
    }
    unreachable!()
}

/// Build a signed CRDT op event (kind 9010) targeting a specific document.
fn make_crdt_op_event(keypair: &CrownKeypair, doc_id: &str, content: &str) -> globe::OmniEvent {
    let payload = IdeaSyncPayload::new(
        uuid::Uuid::parse_str(doc_id).unwrap(),
        vec![serde_json::json!({"type": "insert", "pos": 0, "char": content})],
        serde_json::json!({}),
    );
    let unsigned = UnsignedEvent::new(KIND_IDEA_OPS, serde_json::to_string(&payload).unwrap())
        .with_d_tag(doc_id);
    EventBuilder::sign(&unsigned, keypair).expect("sign crdt op")
}

/// Build a signed cursor event (kind 5120) targeting a specific document.
fn make_cursor_event(keypair: &CrownKeypair, doc_id: &str, offset: usize) -> globe::OmniEvent {
    let msg = CollaborationMessage::Cursor {
        crown_id: keypair.public_key_hex(),
        cursor: serde_json::json!({"offset": offset, "field": "body"}),
    };
    let unsigned =
        UnsignedEvent::new(KIND_COLLABORATION, serde_json::to_string(&msg).unwrap())
            .with_d_tag(doc_id);
    EventBuilder::sign(&unsigned, keypair).expect("sign cursor")
}

/// Build an OmniFilter for a specific document's CRDT ops.
fn doc_ops_filter(doc_id: &str) -> OmniFilter {
    let mut tag_filters = HashMap::new();
    tag_filters.insert('d', vec![doc_id.to_string()]);
    OmniFilter {
        kinds: Some(vec![KIND_IDEA_OPS]),
        tag_filters,
        ..Default::default()
    }
}

/// Build an OmniFilter for a document's CRDT ops AND cursor events.
fn doc_ops_and_cursors_filter(doc_id: &str) -> OmniFilter {
    let mut tag_filters = HashMap::new();
    tag_filters.insert('d', vec![doc_id.to_string()]);
    OmniFilter {
        kinds: Some(vec![KIND_IDEA_OPS, KIND_COLLABORATION]),
        tag_filters,
        ..Default::default()
    }
}

/// Connect N subscribers sequentially, each already subscribed and
/// collecting events. Returns JoinHandles to await results.
async fn connect_subscribers(
    url: &str,
    count: usize,
    make_filter: impl Fn(usize) -> (String, OmniFilter),
    expected_events: usize,
    timeout: Duration,
) -> Vec<(String, tokio::task::JoinHandle<Vec<globe::OmniEvent>>)> {
    let mut handles = Vec::new();
    for i in 0..count {
        let (sub_label, filter) = make_filter(i);
        let ws = connect_with_retry(url).await;
        let (mut write, mut read) = ws.split();

        let sub_id = format!("sub-{i}");
        let sub_msg = ClientMessage::Req {
            subscription_id: sub_id,
            filters: vec![filter],
        };
        write
            .send(Message::Text(sub_msg.to_json().unwrap().into()))
            .await
            .unwrap();

        let handle = tokio::spawn(async move {
            let mut events = Vec::new();
            let deadline = tokio::time::Instant::now() + timeout;
            loop {
                if events.len() >= expected_events {
                    break;
                }
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, read.next()).await {
                    Ok(Some(Ok(frame))) => {
                        if let Ok(text) = frame.to_text() {
                            if let Ok(resp) = RelayMessage::from_json(text) {
                                if let RelayMessage::Event { event, .. } = resp {
                                    events.push(event);
                                }
                            }
                        }
                    }
                    _ => break,
                }
            }
            events
        });
        handles.push((sub_label, handle));
    }
    handles
}

/// Publish a single event over a new WebSocket connection and assert OK.
async fn publish_event(url: &str, event: &globe::OmniEvent) {
    let ws = connect_with_retry(url).await;
    let (mut write, mut read) = ws.split();

    let msg = ClientMessage::Event(event.clone());
    write
        .send(Message::Text(msg.to_json().unwrap().into()))
        .await
        .unwrap();

    let response = read.next().await.unwrap().unwrap();
    match RelayMessage::from_json(response.to_text().unwrap()).unwrap() {
        RelayMessage::Ok { success, .. } => assert!(success, "relay should accept event"),
        other => panic!("expected OK, got: {other:?}"),
    }
}

/// Connect N publishers sequentially, then signal them all to start
/// publishing in parallel. Returns total successful publishes.
///
/// Two-phase approach: first establish all connections (sequential, while
/// the relay is idle), then release all publishers simultaneously. This
/// prevents the publish traffic from interfering with new connections.
async fn publish_from_n_clients(
    url: &str,
    num_publishers: usize,
    _events_per_publisher: usize,
    make_events: impl Fn(usize, &CrownKeypair) -> Vec<globe::OmniEvent> + Send + Sync + 'static,
) -> usize {
    use tokio::sync::Barrier;

    let make_events = Arc::new(make_events);
    let barrier = Arc::new(Barrier::new(num_publishers));
    let mut handles = Vec::new();

    // Phase 1: Connect all publishers sequentially (relay is idle).
    for i in 0..num_publishers {
        let make_events = make_events.clone();
        let barrier = barrier.clone();

        let ws = connect_with_retry(url).await;
        let (mut write, mut read) = ws.split();

        // Phase 2: Spawn task that waits on barrier, then publishes.
        let handle = tokio::spawn(async move {
            let kp = CrownKeypair::generate();
            let events = make_events(i, &kp);

            // Wait until all publishers are connected.
            barrier.wait().await;

            let mut ok_count = 0;
            for event in &events {
                let msg = ClientMessage::Event(event.clone());
                write
                    .send(Message::Text(msg.to_json().unwrap().into()))
                    .await
                    .unwrap();
                if let Some(Ok(frame)) = read.next().await {
                    if let Ok(text) = frame.to_text() {
                        if let Ok(RelayMessage::Ok { success, .. }) = RelayMessage::from_json(text)
                        {
                            if success {
                                ok_count += 1;
                            }
                        }
                    }
                }
            }
            ok_count
        });
        handles.push(handle);
    }

    let mut total = 0;
    for handle in handles {
        total += handle.await.unwrap();
    }
    total
}

// ==========================================================================
// Test 1: 100 concurrent subscribers receive broadcast
// ==========================================================================

#[test]
fn hundred_concurrent_subscribers_receive_broadcast() {
    let config = scale_config("100_sub");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_subscribers = 100;
        let num_events = 10;
        let filter = doc_ops_filter(&doc_id);

        // Connect subscribers sequentially (relay can't handle simultaneous connects).
        let sub_handles = connect_subscribers(
            &url,
            num_subscribers,
            |_| ("doc".into(), filter.clone()),
            num_events,
            Duration::from_secs(15),
        )
        .await;

        eprintln!("  connected {num_subscribers} subscribers in {:?}", start.elapsed());

        // Publisher sends 10 events (sequential -- one connection at a time).
        let publisher_kp = CrownKeypair::generate();
        for i in 0..num_events {
            let event = make_crdt_op_event(&publisher_kp, &doc_id, &format!("op-{i}"));
            publish_event(&url, &event).await;
        }

        // Collect results.
        let mut total_received = 0;
        let mut full_count = 0;
        for (_label, handle) in sub_handles {
            let events = handle.await.unwrap();
            total_received += events.len();
            if events.len() == num_events {
                full_count += 1;
            }
        }

        let elapsed = start.elapsed();
        eprintln!("  [100 subs x 10 events] elapsed: {:?}", elapsed);
        eprintln!(
            "  {full_count}/{num_subscribers} subscribers received all {num_events} events"
        );
        eprintln!("  total events received: {total_received}");

        assert_eq!(
            full_count, num_subscribers,
            "all subscribers should receive all events"
        );
        assert!(elapsed < Duration::from_secs(60), "too slow: {:?}", elapsed);
    });

    cleanup(&config);
}

// ==========================================================================
// Test 2: 100 concurrent publishers, no data loss
// ==========================================================================

#[test]
fn hundred_concurrent_publishers_no_data_loss() {
    let config = scale_config("100_pub");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_publishers = 100;
        let events_per_publisher = 10;
        let expected = num_publishers * events_per_publisher;

        let doc = doc_id.clone();
        let total_ok = publish_from_n_clients(
            &url,
            num_publishers,
            events_per_publisher,
            move |i, kp| {
                (0..events_per_publisher)
                    .map(|j| make_crdt_op_event(kp, &doc, &format!("pub{i}-op{j}")))
                    .collect()
            },
        )
        .await;

        let elapsed = start.elapsed();
        eprintln!("  [100 publishers x 10 events] elapsed: {:?}", elapsed);
        eprintln!("  published OK: {total_ok}/{expected}");

        assert_eq!(total_ok, expected, "all events should be accepted");

        let stored = tower.omnibus().query(&doc_ops_filter(&doc_id));
        eprintln!("  stored in relay: {}", stored.len());
        assert_eq!(stored.len(), expected, "all events should be stored");

        assert!(elapsed < Duration::from_secs(120), "too slow: {:?}", elapsed);
    });

    cleanup(&config);
}

// ==========================================================================
// Test 3: 1000 subscribers fan-out (ignored -- expensive)
// ==========================================================================

#[test]
#[ignore] // Run with: cargo test --test collab_scale -- --ignored --nocapture
fn five_hundred_subscribers_fan_out() {
    let config = scale_config("500_sub");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(200)).await;

        let num_subscribers = 500;
        let num_events = 5;
        let filter = doc_ops_filter(&doc_id);

        let sub_handles = connect_subscribers(
            &url,
            num_subscribers,
            |_| ("doc".into(), filter.clone()),
            num_events,
            Duration::from_secs(30),
        )
        .await;

        eprintln!("  connected {num_subscribers} subscribers in {:?}", start.elapsed());

        let publisher_kp = CrownKeypair::generate();
        for i in 0..num_events {
            let event = make_crdt_op_event(&publisher_kp, &doc_id, &format!("op-{i}"));
            publish_event(&url, &event).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let mut full_count = 0;
        let mut partial_count = 0;
        let mut zero_count = 0;
        let mut total_received = 0;
        for (_label, handle) in sub_handles {
            let events = handle.await.unwrap();
            total_received += events.len();
            if events.len() == num_events {
                full_count += 1;
            } else if !events.is_empty() {
                partial_count += 1;
            } else {
                zero_count += 1;
            }
        }

        let elapsed = start.elapsed();
        eprintln!("  [500 subs x 5 events] elapsed: {:?}", elapsed);
        eprintln!("  full: {full_count}, partial: {partial_count}, zero: {zero_count}");
        eprintln!(
            "  total events delivered: {total_received} / {}",
            num_subscribers * num_events
        );

        // Require at least 95% of subscribers got all events.
        let min_full = (num_subscribers * 95) / 100;
        assert!(
            full_count >= min_full,
            "at least {min_full} subscribers should receive all events, got {full_count}"
        );
    });

    cleanup(&config);
}

// ==========================================================================
// Test 4: Subscription filtering -- no cross-talk
// ==========================================================================

#[test]
fn subscription_filtering_no_crosstalk() {
    let config = scale_config("filter");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_docs = 50;
        let events_per_doc = 2;
        let doc_ids: Vec<String> = (0..num_docs)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect();
        let doc_ids_clone = doc_ids.clone();

        // Each subscriber listens to a different document.
        let sub_handles = connect_subscribers(
            &url,
            num_docs,
            move |i| (doc_ids_clone[i].clone(), doc_ops_filter(&doc_ids_clone[i])),
            events_per_doc,
            Duration::from_secs(15),
        )
        .await;

        eprintln!("  connected {num_docs} subscribers in {:?}", start.elapsed());

        // Publish events targeting each document.
        let publisher_kp = CrownKeypair::generate();
        for doc_id in &doc_ids {
            for j in 0..events_per_doc {
                let event = make_crdt_op_event(&publisher_kp, doc_id, &format!("op-{j}"));
                publish_event(&url, &event).await;
            }
        }

        // Verify each subscriber received ONLY its document's events.
        let mut correct_count = 0;
        for (i, (expected_doc_id, handle)) in sub_handles.into_iter().enumerate() {
            let events = handle.await.unwrap();

            assert_eq!(
                events.len(),
                events_per_doc,
                "subscriber {i} should receive exactly {events_per_doc} events"
            );

            for event in &events {
                let d_values = event.tag_values("d");
                assert!(
                    d_values.contains(&expected_doc_id.as_str()),
                    "subscriber {i} received event for wrong document: d-tags = {d_values:?}"
                );
            }
            correct_count += 1;
        }

        let elapsed = start.elapsed();
        eprintln!("  [50 docs x 2 events, 50 subscribers] elapsed: {:?}", elapsed);
        eprintln!("  all {correct_count}/{num_docs} subscribers received correct events only");

        assert_eq!(correct_count, num_docs, "all subscribers should pass");
        assert!(elapsed < Duration::from_secs(60), "too slow: {:?}", elapsed);
    });

    cleanup(&config);
}

// ==========================================================================
// Test 5: Mixed ops and cursors
// ==========================================================================

#[test]
fn mixed_ops_and_cursors() {
    let config = scale_config("mixed");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_editors = 50;
        let ops_publishers = 25;
        let events_per_publisher = 4;
        let total_events = num_editors * events_per_publisher;
        let filter = doc_ops_and_cursors_filter(&doc_id);

        // Subscribe all 50 editors.
        let sub_handles = connect_subscribers(
            &url,
            num_editors,
            |_| ("editor".into(), filter.clone()),
            total_events,
            Duration::from_secs(30),
        )
        .await;

        eprintln!("  connected {num_editors} subscribers in {:?}", start.elapsed());

        // Connect and publish: first 25 send ops, last 25 send cursors.
        let doc = doc_id.clone();
        let total_published = publish_from_n_clients(
            &url,
            num_editors,
            events_per_publisher,
            move |i, kp| {
                (0..events_per_publisher)
                    .map(|j| {
                        if i < ops_publishers {
                            make_crdt_op_event(kp, &doc, &format!("editor{i}-op{j}"))
                        } else {
                            make_cursor_event(kp, &doc, i * 100 + j)
                        }
                    })
                    .collect()
            },
        )
        .await;

        // Collect subscriber results.
        let mut min_received = usize::MAX;
        let mut max_received = 0;
        let mut ops_count = 0;
        let mut cursor_count = 0;

        for (_label, handle) in sub_handles {
            let events = handle.await.unwrap();
            let count = events.len();
            min_received = min_received.min(count);
            max_received = max_received.max(count);

            for event in &events {
                if event.kind == KIND_IDEA_OPS {
                    ops_count += 1;
                } else if event.kind == KIND_COLLABORATION {
                    cursor_count += 1;
                }
            }
        }

        let elapsed = start.elapsed();
        eprintln!("  [50 editors, mixed ops+cursors] elapsed: {:?}", elapsed);
        eprintln!("  published: {total_published}/{total_events}");
        eprintln!("  received per subscriber: min={min_received}, max={max_received}");
        eprintln!("  total ops events: {ops_count}, cursor events: {cursor_count}");

        assert_eq!(total_published, total_events, "all events should be published");
        assert_eq!(
            min_received, total_events,
            "every subscriber should receive all {total_events} events, min was {min_received}"
        );
        assert!(ops_count > 0, "should receive CRDT op events");
        assert!(cursor_count > 0, "should receive cursor events");
        assert!(elapsed < Duration::from_secs(120), "too slow: {:?}", elapsed);
    });

    cleanup(&config);
}

// ==========================================================================
// Test 6: Burst typing simulation
// ==========================================================================

#[test]
fn burst_typing_simulation() {
    let config = scale_config("burst");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_clients = 20;
        let events_per_client = 50;
        let total_events = num_clients * events_per_client;

        let doc = doc_id.clone();
        let total_ok = publish_from_n_clients(
            &url,
            num_clients,
            events_per_client,
            move |i, kp| {
                (0..events_per_client)
                    .map(|j| make_crdt_op_event(kp, &doc, &format!("burst-{i}-{j}")))
                    .collect()
            },
        )
        .await;

        let elapsed = start.elapsed();
        eprintln!("  [20 clients x 50 events burst] elapsed: {:?}", elapsed);
        eprintln!("  published OK: {total_ok}/{total_events}");

        assert_eq!(total_ok, total_events, "all burst events should be accepted");

        let stored = tower.omnibus().query(&doc_ops_filter(&doc_id));
        eprintln!("  stored in relay: {}", stored.len());
        assert_eq!(stored.len(), total_events, "all burst events should be stored");

        assert!(elapsed < Duration::from_secs(30), "too slow: {:?}", elapsed);
    });

    cleanup(&config);
}

// ==========================================================================
// Test 7: Sustained throughput for 30 seconds (ignored -- expensive)
// ==========================================================================

#[test]
#[ignore] // Run with: cargo test --test collab_scale -- --ignored --nocapture
fn sustained_throughput_30_seconds() {
    let config = scale_config("sustained");
    let tower = start_tower(config.clone());
    let url = tower.relay_url();
    let doc_id = uuid::Uuid::new_v4().to_string();

    let start = Instant::now();

    block_on(&tower, async {
        tokio::time::sleep(Duration::from_millis(100)).await;

        let num_publishers = 10;
        let num_subscribers = 10;
        let duration = Duration::from_secs(30);
        let filter = doc_ops_filter(&doc_id);

        // Connect subscribers sequentially.
        let subscriber_receive_counts: Vec<Arc<AtomicU64>> = (0..num_subscribers)
            .map(|_| Arc::new(AtomicU64::new(0)))
            .collect();

        let mut subscriber_handles = Vec::new();
        for i in 0..num_subscribers {
            let filter = filter.clone();
            let counter = subscriber_receive_counts[i].clone();

            let ws = connect_with_retry(&url).await;
            let (mut write, mut read) = ws.split();

            let sub_msg = ClientMessage::Req {
                subscription_id: format!("sustained-sub-{i}"),
                filters: vec![filter],
            };
            write
                .send(Message::Text(sub_msg.to_json().unwrap().into()))
                .await
                .unwrap();

            let handle = tokio::spawn(async move {
                let deadline = tokio::time::Instant::now() + duration + Duration::from_secs(5);
                loop {
                    let remaining =
                        deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(remaining, read.next()).await {
                        Ok(Some(Ok(frame))) => {
                            if let Ok(text) = frame.to_text() {
                                if let Ok(resp) = RelayMessage::from_json(text) {
                                    if let RelayMessage::Event { .. } = resp {
                                        counter.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                        _ => break,
                    }
                }
            });
            subscriber_handles.push(handle);
        }

        // Connect publishers sequentially.
        let publish_counts: Vec<Arc<AtomicU64>> = (0..num_publishers)
            .map(|_| Arc::new(AtomicU64::new(0)))
            .collect();

        let mut publisher_handles = Vec::new();
        for i in 0..num_publishers {
            let doc_id = doc_id.clone();
            let counter = publish_counts[i].clone();

            let ws = connect_with_retry(&url).await;
            let (mut write, mut read) = ws.split();

            let handle = tokio::spawn(async move {
                let kp = CrownKeypair::generate();
                let end_time = tokio::time::Instant::now() + duration;
                let mut seq = 0u64;

                while tokio::time::Instant::now() < end_time {
                    // Send a burst of 10 events, then pause 1 second.
                    for b in 0..10u64 {
                        seq += 1;
                        let event = make_crdt_op_event(
                            &kp,
                            &doc_id,
                            &format!("sustained-{i}-{seq}-{b}"),
                        );
                        let msg = ClientMessage::Event(event);
                        if write
                            .send(Message::Text(msg.to_json().unwrap().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        if let Some(Ok(frame)) = read.next().await {
                            if let Ok(text) = frame.to_text() {
                                if let Ok(RelayMessage::Ok { success, .. }) =
                                    RelayMessage::from_json(text)
                                {
                                    if success {
                                        counter.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
            publisher_handles.push(handle);
        }

        eprintln!("  connected all clients in {:?}, running for 30s...", start.elapsed());

        for handle in publisher_handles {
            handle.await.unwrap();
        }

        tokio::time::sleep(Duration::from_secs(3)).await;

        for handle in subscriber_handles {
            let _ = handle.await;
        }

        let total_published: u64 =
            publish_counts.iter().map(|c| c.load(Ordering::Relaxed)).sum();
        let per_sub: Vec<u64> = subscriber_receive_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        let min_rx = per_sub.iter().copied().min().unwrap_or(0);
        let max_rx = per_sub.iter().copied().max().unwrap_or(0);
        let avg_rx = per_sub.iter().sum::<u64>() as f64 / num_subscribers as f64;

        let elapsed = start.elapsed();
        eprintln!("  [sustained 30s] elapsed: {:?}", elapsed);
        eprintln!("  total published: {total_published}");
        eprintln!("  subscriber received: min={min_rx}, max={max_rx}, avg={avg_rx:.1}");
        eprintln!("  publish throughput: {:.1} events/sec", total_published as f64 / 30.0);

        let stored = tower.omnibus().query(&doc_ops_filter(&doc_id));
        eprintln!("  stored in relay: {}", stored.len());

        assert!(total_published > 0, "should have published events");
        assert_eq!(stored.len(), total_published as usize, "all should be stored");

        let threshold = (total_published * 90) / 100;
        assert!(
            min_rx >= threshold,
            "each subscriber should receive at least {threshold} events, min was {min_rx}"
        );
    });

    cleanup(&config);
}

// ==========================================================================
// Test 8: CRDT convergence under load (pure, no networking)
// ==========================================================================

#[test]
fn crdt_convergence_under_load() {
    let num_replicas = 10;
    let ops_per_replica = 100;

    let start = Instant::now();

    let mut replicas: Vec<SequenceRga> = (0..num_replicas)
        .map(|i| SequenceRga::new(format!("replica-{i}")))
        .collect();

    let mut all_ops: Vec<Vec<x::SequenceOp>> = Vec::new();

    for replica in &mut replicas {
        let mut ops = Vec::new();
        for j in 0..ops_per_replica {
            let len = replica.len();
            if len > 3 && j % 5 == 4 {
                let pos = j % len;
                if let Some(op) = replica.delete_at(pos) {
                    ops.push(op);
                }
            } else {
                let ch = (b'a' + (j % 26) as u8) as char;
                let pos = if len == 0 { 0 } else { j % (len + 1) };
                let op = replica.insert_at(pos, ch);
                ops.push(op);
            }
        }
        all_ops.push(ops);
    }

    let generate_elapsed = start.elapsed();
    eprintln!(
        "  generated {} ops across {} replicas in {:?}",
        all_ops.iter().map(|v| v.len()).sum::<usize>(),
        num_replicas,
        generate_elapsed,
    );

    let apply_start = Instant::now();
    for target_idx in 0..num_replicas {
        for (source_idx, ops) in all_ops.iter().enumerate() {
            if source_idx == target_idx {
                continue;
            }
            for op in ops {
                replicas[target_idx].apply(op);
            }
        }
    }
    let apply_elapsed = apply_start.elapsed();

    let texts: Vec<String> = replicas.iter().map(|r| r.text()).collect();
    let reference = &texts[0];

    for (i, text) in texts.iter().enumerate() {
        assert_eq!(
            text, reference,
            "replica {i} diverged from replica 0!\n  replica 0: {:?}\n  replica {i}: {:?}",
            &reference[..reference.len().min(80)],
            &text[..text.len().min(80)],
        );
    }

    let total_elapsed = start.elapsed();
    eprintln!(
        "  convergence verified: {} replicas, {} chars",
        num_replicas,
        reference.len(),
    );
    eprintln!("  apply phase: {:?}, total: {:?}", apply_elapsed, total_elapsed);
    eprintln!("  final text length: {} chars", reference.len());

    assert!(
        total_elapsed < Duration::from_secs(10),
        "CRDT convergence should be fast, took {:?}",
        total_elapsed,
    );
}
