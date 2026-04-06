use crown::CrownKeypair;
use futures_util::{SinkExt, StreamExt};
use globe::*;
use tokio_tungstenite::tungstenite::Message;

#[test]
fn event_sign_verify_round_trip() {
    let kp = CrownKeypair::generate();
    let unsigned = UnsignedEvent::new(1, "sovereign data")
        .with_application_tag("omnidea");

    let event = EventBuilder::sign(&unsigned, &kp).unwrap();

    // Structural validation.
    assert!(event.validate().is_ok());
    assert_eq!(event.kind, 1);
    assert_eq!(event.content, "sovereign data");
    assert!(event.has_tag("application", "omnidea"));

    // Cryptographic verification.
    let valid = EventBuilder::verify(&event).unwrap();
    assert!(valid);
}

#[test]
fn standard_event_builders() {
    let kp = CrownKeypair::generate();

    // Profile.
    let profile = EventBuilder::profile("Sam", Some("Builder"), None, &kp).unwrap();
    assert_eq!(profile.kind, 0);
    assert!(profile.content.contains("Sam"));
    assert!(EventBuilder::verify(&profile).unwrap());

    // Text note.
    let note = EventBuilder::text_note("Hello Omnidea!", &kp).unwrap();
    assert_eq!(note.kind, 1);
    assert!(EventBuilder::verify(&note).unwrap());

    // Contact list.
    let contacts = EventBuilder::contact_list(&["pub1", "pub2"], &kp).unwrap();
    assert_eq!(contacts.kind, 3);
    assert_eq!(contacts.p_tags().len(), 2);
    assert!(EventBuilder::verify(&contacts).unwrap());
}

#[test]
fn filter_matching_comprehensive() {
    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("test", &kp).unwrap();

    // Match by kind.
    let kind_filter = OmniFilter {
        kinds: Some(vec![1]),
        ..Default::default()
    };
    assert!(kind_filter.matches(&event));

    // Match by author.
    let author_filter = OmniFilter {
        authors: Some(vec![kp.public_key_hex()]),
        ..Default::default()
    };
    assert!(author_filter.matches(&event));

    // Combined AND: kind + author.
    let combined = OmniFilter {
        kinds: Some(vec![1]),
        authors: Some(vec![kp.public_key_hex()]),
        ..Default::default()
    };
    assert!(combined.matches(&event));

    // Wrong kind fails.
    let wrong_kind = OmniFilter {
        kinds: Some(vec![7000]),
        ..Default::default()
    };
    assert!(!wrong_kind.matches(&event));
}

#[test]
fn protocol_encode_decode_round_trip() {
    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("protocol test", &kp).unwrap();

    // Client EVENT.
    let client_msg = ClientMessage::Event(event.clone());
    let json = client_msg.to_json().unwrap();
    assert!(json.starts_with("[\"EVENT\""));

    // Client REQ.
    let req = ClientMessage::Req {
        subscription_id: "sub-1".into(),
        filters: vec![OmniFilter::for_profile(&kp.public_key_hex())],
    };
    let json = req.to_json().unwrap();
    assert!(json.contains("\"REQ\""));
    assert!(json.contains("sub-1"));

    // Relay EVENT decode.
    let relay_json = format!(
        r#"["EVENT","sub-1",{}]"#,
        serde_json::to_string(&event).unwrap()
    );
    let relay_msg = RelayMessage::from_json(&relay_json).unwrap();
    match relay_msg {
        RelayMessage::Event {
            subscription_id,
            event: e,
        } => {
            assert_eq!(subscription_id, "sub-1");
            assert_eq!(e.content, "protocol test");
        }
        _ => panic!("expected RelayMessage::Event"),
    }

    // Relay STORED.
    let stored = RelayMessage::from_json(r#"["STORED","sub-1"]"#).unwrap();
    assert_eq!(stored, RelayMessage::Stored("sub-1".into()));

    // Relay OK.
    let ok = RelayMessage::from_json(r#"["OK","eid",true,"accepted"]"#).unwrap();
    match ok {
        RelayMessage::Ok {
            success, message, ..
        } => {
            assert!(success);
            assert_eq!(message, Some("accepted".into()));
        }
        _ => panic!("expected RelayMessage::Ok"),
    }
}

#[test]
fn auth_challenge_response_verify() {
    let kp = CrownKeypair::generate();
    let url = url::Url::parse("wss://relay.omnidea.co").unwrap();

    let response = globe::auth::create_auth_response("test_challenge", &url, &kp).unwrap();

    assert_eq!(response.kind, globe::kind::AUTH_EVENT);
    assert!(EventBuilder::verify(&response).unwrap());

    let valid =
        globe::auth::verify_auth_response(&response, "test_challenge", &url).unwrap();
    assert!(valid);

    // Wrong challenge fails.
    let invalid =
        globe::auth::verify_auth_response(&response, "wrong_challenge", &url).unwrap();
    assert!(!invalid);
}

#[test]
fn name_claim_resolve_transfer() {
    let owner = CrownKeypair::generate();
    let buyer = CrownKeypair::generate();

    // Claim.
    let claim = NameBuilder::claim("sam.com", &owner).unwrap();
    assert_eq!(claim.kind, globe::kind::NAME_CLAIM);

    let record = globe::name::parse_name_record(&claim).unwrap();
    assert_eq!(record.name, "sam.com");
    assert_eq!(record.owner, owner.public_key_hex());

    // Resolve filter.
    let filter = globe::name::resolve_filter("sam.com");
    assert!(filter.matches(&claim));

    // Transfer.
    let transfer =
        NameBuilder::transfer("sam.com", &buyer.public_key_hex(), &owner).unwrap();
    assert_eq!(transfer.kind, globe::kind::NAME_TRANSFER);
    assert!(transfer.has_tag("target", &buyer.public_key_hex()));
}

#[test]
fn kind_subsystem_routing_all_abcs() {
    use globe::kind::*;

    // Standard.
    assert_eq!(subsystem_for_kind(PROFILE), Subsystem::Standard);
    assert_eq!(subsystem_for_kind(TEXT_NOTE), Subsystem::Standard);

    // All 26 ABCs at their range start.
    assert_eq!(subsystem_for_kind(1000), Subsystem::Advisor);
    assert_eq!(subsystem_for_kind(2000), Subsystem::Bulwark);
    assert_eq!(subsystem_for_kind(3000), Subsystem::Crown);
    assert_eq!(subsystem_for_kind(4000), Subsystem::Divinity);
    assert_eq!(subsystem_for_kind(5000), Subsystem::Equipment);
    assert_eq!(subsystem_for_kind(6000), Subsystem::Fortune);
    assert_eq!(subsystem_for_kind(7000), Subsystem::Globe);
    assert_eq!(subsystem_for_kind(8000), Subsystem::Hall);
    assert_eq!(subsystem_for_kind(9000), Subsystem::Ideas);
    assert_eq!(subsystem_for_kind(10000), Subsystem::Jail);
    assert_eq!(subsystem_for_kind(11000), Subsystem::Kingdom);
    assert_eq!(subsystem_for_kind(12000), Subsystem::Lingo);
    assert_eq!(subsystem_for_kind(13000), Subsystem::Magic);
    assert_eq!(subsystem_for_kind(14000), Subsystem::Nexus);
    assert_eq!(subsystem_for_kind(15000), Subsystem::Oracle);
    assert_eq!(subsystem_for_kind(16000), Subsystem::Polity);
    assert_eq!(subsystem_for_kind(17000), Subsystem::Quest);
    assert_eq!(subsystem_for_kind(18000), Subsystem::Regalia);
    assert_eq!(subsystem_for_kind(19000), Subsystem::Sentinal);
    assert_eq!(subsystem_for_kind(20000), Subsystem::Throne);
    assert_eq!(subsystem_for_kind(21000), Subsystem::Universe);
    assert_eq!(subsystem_for_kind(22000), Subsystem::Vault);
    assert_eq!(subsystem_for_kind(23000), Subsystem::World);
    assert_eq!(subsystem_for_kind(24000), Subsystem::X);
    assert_eq!(subsystem_for_kind(25000), Subsystem::Yoke);
    assert_eq!(subsystem_for_kind(26000), Subsystem::Zeitgeist);

    // Special ranges.
    assert_eq!(subsystem_for_kind(35000), Subsystem::Replaceable);
    assert_eq!(subsystem_for_kind(45000), Subsystem::Parameterized);
    assert_eq!(subsystem_for_kind(60000), Subsystem::Extension);
}

#[test]
fn event_deduplication_via_pool() {
    let pool = RelayPool::new(GlobeConfig::default());
    let url = url::Url::parse("wss://relay.example.com").unwrap();

    let event = OmniEvent {
        id: "a".repeat(64),
        author: "b".repeat(64),
        created_at: 12345,
        kind: 1,
        tags: vec![],
        content: "test".into(),
        sig: "c".repeat(128),
    };

    // First: not duplicate, processed.
    assert!(pool.process_event(event.clone(), url.clone()));

    // Second: duplicate, rejected.
    assert!(!pool.process_event(event.clone(), url.clone()));

    // Different event: not duplicate.
    let mut event2 = event;
    event2.id = "d".repeat(64);
    assert!(pool.process_event(event2, url));
}

#[tokio::test]
async fn client_server_end_to_end() {
    // Start a relay server on a random port.
    let (server, addr) = RelayServer::start_on_available_port().await.unwrap();

    // Give the server a moment to start accepting.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Connect a WebSocket client.
    let url = format!("ws://{addr}");
    let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut write, mut read) = ws.split();

    // Create and publish a signed event.
    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("Hello from the sovereign internet!", &kp).unwrap();

    let publish_msg = ClientMessage::Event(event.clone());
    write.send(Message::Text(publish_msg.to_json().unwrap().into())).await.unwrap();

    // Read the OK response.
    let response = read.next().await.unwrap().unwrap();
    let relay_msg = RelayMessage::from_json(response.to_text().unwrap()).unwrap();
    match relay_msg {
        RelayMessage::Ok { event_id, success, .. } => {
            assert!(success);
            assert_eq!(event_id, event.id);
        }
        other => panic!("expected OK, got: {other:?}"),
    }

    // Subscribe to kind 1 events.
    let sub_msg = ClientMessage::Req {
        subscription_id: "sub-1".into(),
        filters: vec![OmniFilter {
            kinds: Some(vec![1]),
            ..Default::default()
        }],
    };
    write.send(Message::Text(sub_msg.to_json().unwrap().into())).await.unwrap();

    // Should receive the stored event.
    let stored_event = read.next().await.unwrap().unwrap();
    let relay_msg = RelayMessage::from_json(stored_event.to_text().unwrap()).unwrap();
    match relay_msg {
        RelayMessage::Event { subscription_id, event: e } => {
            assert_eq!(subscription_id, "sub-1");
            assert_eq!(e.content, "Hello from the sovereign internet!");
        }
        other => panic!("expected EVENT, got: {other:?}"),
    }

    // Should receive STORED signal.
    let stored_signal = read.next().await.unwrap().unwrap();
    let relay_msg = RelayMessage::from_json(stored_signal.to_text().unwrap()).unwrap();
    assert_eq!(relay_msg, RelayMessage::Stored("sub-1".into()));

    // Verify the event is in the server's store.
    assert_eq!(server.store().len(), 1);

    // Close subscription.
    let close_msg = ClientMessage::Close("sub-1".into());
    write.send(Message::Text(close_msg.to_json().unwrap().into())).await.unwrap();
}

#[tokio::test]
async fn server_broadcasts_live_events() {
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let url = format!("ws://{addr}");

    // Client A subscribes.
    let (ws_a, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut write_a, mut read_a) = ws_a.split();

    let sub_msg = ClientMessage::Req {
        subscription_id: "sub-live".into(),
        filters: vec![OmniFilter {
            kinds: Some(vec![1]),
            ..Default::default()
        }],
    };
    write_a.send(Message::Text(sub_msg.to_json().unwrap().into())).await.unwrap();

    // Read the STORED signal (no stored events yet).
    let msg = read_a.next().await.unwrap().unwrap();
    let relay_msg = RelayMessage::from_json(msg.to_text().unwrap()).unwrap();
    assert_eq!(relay_msg, RelayMessage::Stored("sub-live".into()));

    // Client B publishes an event.
    let (ws_b, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut write_b, mut read_b) = ws_b.split();

    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("Live broadcast!", &kp).unwrap();
    let publish_msg = ClientMessage::Event(event.clone());
    write_b.send(Message::Text(publish_msg.to_json().unwrap().into())).await.unwrap();

    // Client B gets OK.
    let ok_msg = read_b.next().await.unwrap().unwrap();
    let relay_msg = RelayMessage::from_json(ok_msg.to_text().unwrap()).unwrap();
    match relay_msg {
        RelayMessage::Ok { success, .. } => assert!(success),
        other => panic!("expected OK, got: {other:?}"),
    }

    // Client A should receive the live event.
    let live_msg = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        read_a.next(),
    ).await.unwrap().unwrap().unwrap();

    let relay_msg = RelayMessage::from_json(live_msg.to_text().unwrap()).unwrap();
    match relay_msg {
        RelayMessage::Event { subscription_id, event: e } => {
            assert_eq!(subscription_id, "sub-live");
            assert_eq!(e.content, "Live broadcast!");
        }
        other => panic!("expected live EVENT, got: {other:?}"),
    }
}

// -- Asset HTTP integration tests --

#[tokio::test]
async fn asset_http_put_get_head() {
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // Start a relay with assets enabled.
    let config = ServerConfig {
        enable_assets: true,
        ..Default::default()
    };
    let (_server, addr) = RelayServer::start_with_config(config).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let data = b"sovereign binary asset data for omnidea";
    let hash = hex::encode(Sha256::digest(data));

    // PUT the asset.
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!(
            "PUT /asset/{hash} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            data.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        stream.write_all(data).await.unwrap();

        let mut response = vec![0u8; 4096];
        let n = stream.read(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response[..n]).unwrap();
        assert!(
            response_text.contains("201") || response_text.contains("200"),
            "PUT failed: {response_text}"
        );
    }

    // GET the asset back.
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!("GET /asset/{hash} HTTP/1.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response).unwrap();
        assert!(response_text.contains("200"), "GET failed: {response_text}");
        assert!(response_text.contains(&hash));
        // Body should contain our original data.
        assert!(response.windows(data.len()).any(|w| w == data));
    }

    // HEAD — check existence.
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!("HEAD /asset/{hash} HTTP/1.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = vec![0u8; 4096];
        let n = stream.read(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_text.contains("200"), "HEAD failed: {response_text}");
    }

    // GET nonexistent — 404.
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let fake_hash = "b".repeat(64);
        let request = format!("GET /asset/{fake_hash} HTTP/1.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = vec![0u8; 4096];
        let n = stream.read(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response[..n]).unwrap();
        assert!(response_text.contains("404"), "Expected 404: {response_text}");
    }

    // WebSocket still works alongside HTTP assets.
    {
        let url = format!("ws://{addr}");
        let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut write, mut read) = ws.split();

        let kp = CrownKeypair::generate();
        let event = EventBuilder::text_note("WebSocket still works!", &kp).unwrap();
        let msg = ClientMessage::Event(event.clone());
        write.send(Message::Text(msg.to_json().unwrap().into())).await.unwrap();

        let response = read.next().await.unwrap().unwrap();
        let relay_msg = RelayMessage::from_json(response.to_text().unwrap()).unwrap();
        match relay_msg {
            RelayMessage::Ok { success, .. } => assert!(success),
            other => panic!("expected OK, got: {other:?}"),
        }
    }
}

#[tokio::test]
async fn asset_pull_through_caching() {
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // Start Relay A — the origin, has the asset.
    let config_a = ServerConfig {
        enable_assets: true,
        ..Default::default()
    };
    let (server_a, addr_a) = RelayServer::start_with_config(config_a).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Start Relay B — the edge, peers with Relay A for pull-through.
    let peer_url_a = url::Url::parse(&format!("ws://{addr_a}")).unwrap();
    let config_b = ServerConfig {
        enable_assets: true,
        asset_peer_urls: vec![peer_url_a],
        ..Default::default()
    };
    let (_server_b, addr_b) = RelayServer::start_with_config(config_b).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Upload asset to Relay A.
    let data = b"pull-through caching test data for the sovereign CDN";
    let hash = hex::encode(Sha256::digest(data));
    {
        let mut stream = TcpStream::connect(addr_a).await.unwrap();
        let request = format!(
            "PUT /asset/{hash} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            data.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        stream.write_all(data).await.unwrap();
        let mut resp = vec![0u8; 4096];
        let n = stream.read(&mut resp).await.unwrap();
        let resp_text = std::str::from_utf8(&resp[..n]).unwrap();
        assert!(resp_text.contains("201"), "PUT to A failed: {resp_text}");
    }

    // Verify Relay A has it.
    assert!(server_a.asset_store().exists(&hash));

    // GET from Relay B — it doesn't have it, should pull from A.
    {
        let mut stream = TcpStream::connect(addr_b).await.unwrap();
        let request = format!("GET /asset/{hash} HTTP/1.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response).unwrap();
        assert!(
            response_text.contains("200"),
            "Pull-through GET from B failed: {response_text}"
        );
        // Verify the actual data is in the response.
        assert!(response.windows(data.len()).any(|w| w == data));
    }

    // Relay B should now have it cached.
    assert!(_server_b.asset_store().exists(&hash));

    // Second GET from B should be served from local cache (no network to A).
    {
        let mut stream = TcpStream::connect(addr_b).await.unwrap();
        let request = format!("GET /asset/{hash} HTTP/1.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response_text = std::str::from_utf8(&response).unwrap();
        assert!(
            response_text.contains("200"),
            "Cached GET from B failed: {response_text}"
        );
    }
}

// -- RelayHandle WebSocket integration tests --

#[tokio::test]
async fn relay_handle_publishes_and_gets_ok() {
    use globe::client::connection::RelayHandle;
    use globe::health::ConnectionState;
    use std::sync::Arc;

    // Start a relay server.
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Create a RelayHandle pointing at the local server.
    let url = url::Url::parse(&format!("ws://{addr}")).unwrap();
    let config = Arc::new(GlobeConfig::default());
    let handle = RelayHandle::new(url, config);

    // Wait for connection.
    let mut state_rx = handle.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *state_rx.borrow_and_update() != ConnectionState::Connected {
            state_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("timed out waiting for connection");

    assert_eq!(handle.state(), ConnectionState::Connected);

    // Publish a signed event via the handle.
    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("Published via RelayHandle!", &kp).unwrap();
    let event_id = event.id.clone();

    let result = handle.publish(event).await;
    assert!(result.is_ok(), "publish failed: {result:?}");

    // Verify the server stored it.
    assert_eq!(_server.store().len(), 1);
    let stored = _server.store().query(&OmniFilter {
        kinds: Some(vec![1]),
        ..Default::default()
    });
    assert_eq!(stored[0].id, event_id);

    handle.disconnect().await.unwrap();
}

#[tokio::test]
async fn relay_handle_subscribes_and_receives_live_events() {
    use globe::client::connection::{RelayEvent, RelayHandle};
    use globe::health::ConnectionState;
    use std::sync::Arc;

    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = url::Url::parse(&format!("ws://{addr}")).unwrap();
    let config = Arc::new(GlobeConfig::default());

    // Handle A subscribes.
    let handle_a = RelayHandle::new(url.clone(), config.clone());
    let mut state_a = handle_a.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *state_a.borrow_and_update() != ConnectionState::Connected {
            state_a.changed().await.unwrap();
        }
    })
    .await
    .expect("handle A timed out connecting");

    handle_a
        .subscribe(
            "live-sub".into(),
            vec![OmniFilter {
                kinds: Some(vec![1]),
                ..Default::default()
            }],
        )
        .await
        .unwrap();

    // Subscribe to the event broadcast.
    let mut event_rx = handle_a.subscribe_events();

    // Wait for STORED marker (no stored events yet, so it comes immediately).
    let stored = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match event_rx.recv().await {
                Ok(RelayEvent::StoredComplete { subscription_id, .. }) => {
                    return subscription_id;
                }
                Ok(RelayEvent::Event { .. }) => continue,
                Err(_) => panic!("event channel error"),
            }
        }
    })
    .await
    .expect("timed out waiting for STORED");
    assert_eq!(stored, "live-sub");

    // Handle B publishes.
    let handle_b = RelayHandle::new(url, config);
    let mut state_b = handle_b.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *state_b.borrow_and_update() != ConnectionState::Connected {
            state_b.changed().await.unwrap();
        }
    })
    .await
    .expect("handle B timed out connecting");

    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("Live event via handles!", &kp).unwrap();
    let expected_content = event.content.clone();
    handle_b.publish(event).await.unwrap();

    // Handle A should receive the live event.
    let received = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match event_rx.recv().await {
                Ok(RelayEvent::Event { event, .. }) => return event,
                Ok(_) => continue,
                Err(_) => panic!("event channel error"),
            }
        }
    })
    .await
    .expect("timed out waiting for live event");

    assert_eq!(received.content, expected_content);

    handle_a.disconnect().await.unwrap();
    handle_b.disconnect().await.unwrap();
}

#[tokio::test]
async fn relay_handle_reconnects_after_disconnect() {
    use globe::client::connection::RelayHandle;
    use globe::health::ConnectionState;
    use std::sync::Arc;
    use tokio::sync::Notify;

    // Set up a raw TCP listener that accepts, closes, then re-accepts.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let reconnected = Arc::new(Notify::new());
    let reconnected_clone = reconnected.clone();

    tokio::spawn(async move {
        // Accept first connection, then close it.
        let (stream, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        drop(ws); // Close the WebSocket — client will detect this.

        // Accept reconnection.
        let (stream2, _) = listener.accept().await.unwrap();
        let _ws2 = tokio_tungstenite::accept_async(stream2).await.unwrap();
        reconnected_clone.notify_one();
        // Keep alive for the test.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    });

    // Connect via RelayHandle with fast reconnect.
    let url = url::Url::parse(&format!("ws://{addr}")).unwrap();
    let config = Arc::new(GlobeConfig {
        reconnect_min_delay: std::time::Duration::from_millis(100),
        reconnect_max_delay: std::time::Duration::from_secs(1),
        ..Default::default()
    });
    let handle = RelayHandle::new(url, config);

    // Wait for initial connection.
    let mut state_rx = handle.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *state_rx.borrow_and_update() != ConnectionState::Connected {
            state_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("timed out waiting for initial connection");

    // Wait for disconnect detection + reconnection.
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        // Wait for disconnection.
        loop {
            state_rx.changed().await.unwrap();
            let state = state_rx.borrow().clone();
            if !matches!(state, ConnectionState::Connected) {
                break;
            }
        }
        // Wait for reconnection.
        loop {
            state_rx.changed().await.unwrap();
            if *state_rx.borrow() == ConnectionState::Connected {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for reconnection");

    assert_eq!(handle.state(), ConnectionState::Connected);
    handle.disconnect().await.unwrap();
}

#[tokio::test]
async fn gospel_peer_syncs_names_over_websocket() {
    use globe::client::connection::RelayHandle;
    use globe::gospel::{GospelConfig, GospelPeer, GospelRegistry};
    use globe::health::ConnectionState;
    use std::sync::Arc;

    // Start a relay server.
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = url::Url::parse(&format!("ws://{addr}")).unwrap();
    let config = Arc::new(GlobeConfig::default());

    // Publish gospel events (name claims) to the server directly.
    let raw_handle = RelayHandle::new(url.clone(), config.clone());
    let mut state_rx = raw_handle.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *state_rx.borrow_and_update() != ConnectionState::Connected {
            state_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("raw handle timed out connecting");

    let alice_kp = CrownKeypair::generate();
    let name_claim = EventBuilder::sign(
        &globe::UnsignedEvent::new(globe::kind::NAME_CLAIM, "")
            .with_d_tag("alice.com"),
        &alice_kp,
    )
    .unwrap();
    raw_handle.publish(name_claim.clone()).await.unwrap();

    let bob_kp = CrownKeypair::generate();
    let hint_event = EventBuilder::sign(
        &globe::UnsignedEvent::new(
            globe::kind::RELAY_HINT,
            r#"{"relays":["wss://bob-relay.com"]}"#,
        )
        .with_d_tag("relay-hints"),
        &bob_kp,
    )
    .unwrap();
    raw_handle.publish(hint_event.clone()).await.unwrap();

    raw_handle.disconnect().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Create a GospelPeer that connects to the server and evangelizes.
    let gospel_config = GospelConfig::default();
    let local_registry = GospelRegistry::new(&gospel_config);
    assert_eq!(local_registry.name_count(), 0);
    assert_eq!(local_registry.hint_count(), 0);

    let mut peer = GospelPeer::new(url, config, globe::GospelTier::all());

    // Wait for the peer's handle to connect.
    let peer_handle = peer.handle().clone();
    let mut peer_state = peer_handle.watch_state();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while *peer_state.borrow_and_update() != ConnectionState::Connected {
            peer_state.changed().await.unwrap();
        }
    })
    .await
    .expect("gospel peer timed out connecting");

    // Evangelize — should pull name claim + relay hint from server.
    let (received, sent) = peer.evangelize(&local_registry).await.unwrap();

    assert!(received >= 2, "expected >= 2 received, got {received}");
    assert_eq!(sent, 0); // Local registry was empty, nothing to send.

    // Verify the local registry now has the records.
    assert_eq!(local_registry.name_count(), 1);
    assert_eq!(local_registry.hint_count(), 1);
    assert!(local_registry.lookup_name("alice.com").is_some());

    // Verify peer state updated.
    assert_eq!(peer.state().evangelize_count, 1);
    assert!(peer.state().events_received >= 2);

    peer.disconnect().await.unwrap();
}

#[tokio::test]
async fn pool_forwards_live_events() {
    use globe::client::pool::RelayPool;

    // Start a relay server.
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = url::Url::parse(&format!("ws://{addr}")).unwrap();

    // Create a pool and add the relay.
    let mut pool = RelayPool::new(GlobeConfig::default());
    pool.add_relay(url.clone()).unwrap();

    // Subscribe via the pool.
    let (_sub_id, mut pool_rx) = pool.subscribe(vec![OmniFilter {
        kinds: Some(vec![1]),
        ..Default::default()
    }]);

    // Give handles time to connect and subscribe.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Publish from a separate raw WebSocket client.
    let (ws, _) =
        tokio_tungstenite::connect_async(&format!("ws://{addr}"))
            .await
            .unwrap();
    let (mut write, mut read) = ws.split();

    let kp = CrownKeypair::generate();
    let event = EventBuilder::text_note("Pool forwarding test!", &kp).unwrap();
    let expected_id = event.id.clone();
    let msg = ClientMessage::Event(event);
    write
        .send(Message::Text(msg.to_json().unwrap().into()))
        .await
        .unwrap();

    // Read OK from server.
    let _ = read.next().await;

    // Pool should receive the forwarded event.
    let pool_event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match pool_rx.recv().await {
                Ok(pe) => return pe,
                Err(_) => continue,
            }
        }
    })
    .await
    .expect("timed out waiting for pool event");

    assert_eq!(pool_event.event.id, expected_id);
    assert_eq!(pool_event.event.content, "Pool forwarding test!");
}

#[tokio::test]
async fn asset_http_hash_mismatch_rejected() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = ServerConfig {
        enable_assets: true,
        ..Default::default()
    };
    let (_server, addr) = RelayServer::start_with_config(config).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let data = b"some data";
    let wrong_hash = "a".repeat(64); // Not the real hash.

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "PUT /asset/{wrong_hash} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        data.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.write_all(data).await.unwrap();

    let mut response = vec![0u8; 4096];
    let n = stream.read(&mut response).await.unwrap();
    let response_text = std::str::from_utf8(&response[..n]).unwrap();
    assert!(
        response_text.contains("409"),
        "Expected 409 hash mismatch: {response_text}"
    );
}
