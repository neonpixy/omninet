//! Pulse FFI — full-stack proof of life.
//!
//! One function exercises all 15 Rust crates and returns a JSON report.
//! Each step is wrapped in a catch so one failure doesn't block others.

use std::ffi::c_char;

use serde::Serialize;

use regalia::Material;

use crate::helpers::json_to_c;

/// Result for a single crate demo step.
#[derive(Serialize)]
struct CrateResult {
    crate_name: String,
    letter: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    data: serde_json::Value,
}

/// The full Pulse demo report.
#[derive(Serialize)]
struct PulseReport {
    version: String,
    crates_tested: usize,
    crates_passed: usize,
    results: Vec<CrateResult>,
}

fn run_step(
    crate_name: &str,
    letter: &str,
    f: impl FnOnce() -> Result<serde_json::Value, String>,
) -> CrateResult {
    match f() {
        Ok(data) => CrateResult {
            crate_name: crate_name.into(),
            letter: letter.into(),
            success: true,
            error: None,
            data,
        },
        Err(e) => CrateResult {
            crate_name: crate_name.into(),
            letter: letter.into(),
            success: false,
            error: Some(e),
            data: serde_json::Value::Null,
        },
    }
}

/// Run the full Pulse demo. Returns a JSON report (caller must free via `divi_free_string`).
#[unsafe(no_mangle)]
pub extern "C" fn divi_pulse_run_demo() -> *mut c_char {
    let mut results = Vec::new();

    // 1. X — Value + VectorClock
    results.push(run_step("X", "X", || {
        let value = x::Value::from("Omnidea lives");
        let mut clock = x::VectorClock::new();
        clock.increment("pulse");
        clock.increment("pulse");
        Ok(serde_json::json!({
            "value": value.as_str().unwrap_or("?"),
            "clock_ticks": clock.count_for("pulse"),
        }))
    }));

    // 2. Crown — CrownKeypair + Profile
    results.push(run_step("Crown", "C", || {
        let keypair = crown::CrownKeypair::generate();
        let mut profile = crown::Profile::empty();
        profile.display_name = Some("Pulse Demo".into());
        Ok(serde_json::json!({
            "crown_id": keypair.crown_id(),
            "display_name": profile.display_name,
        }))
    }));

    // 3. Ideas — Digit with Header
    results.push(run_step("Ideas", "I", || {
        let digit = ideas::Digit::new(
            "text".into(),
            x::Value::from("Hello from Pulse"),
            "pulse-demo".into(),
        )
        .map_err(|e| format!("{e}"))?;

        let header = ideas::Header::create(
            "pulse-demo-key".into(),
            "pulse-demo-sig".into(),
            digit.id(),
            ideas::header::KeySlot::Internal(ideas::header::InternalKeySlot {
                key_id: "pulse".into(),
                wrapped_key: "demo".into(),
            }),
        );

        Ok(serde_json::json!({
            "digit_id": digit.id().to_string(),
            "content_type": "text",
            "header_version": header.version,
        }))
    }));

    // 4. Sentinal — PBKDF2 + AES-256-GCM
    results.push(run_step("Sentinal", "S", || {
        let (master_key, _salt) =
            sentinal::key_derivation::derive_master_key("pulse-password", None)
                .map_err(|e| format!("{e}"))?;

        let plaintext = b"Dignity, Sovereignty, Consent";
        let encrypted = sentinal::encryption::encrypt(plaintext, master_key.expose())
            .map_err(|e| format!("{e}"))?;
        let decrypted = sentinal::encryption::decrypt(&encrypted, master_key.expose())
            .map_err(|e| format!("{e}"))?;

        Ok(serde_json::json!({
            "encrypted_len": encrypted.combined().len(),
            "decrypted_matches": decrypted == plaintext,
        }))
    }));

    // 5. Vault — create, unlock, register, lock
    results.push(run_step("Vault", "V", || {
        let tmp = tempfile::tempdir().map_err(|e| format!("{e}"))?;
        let mut v = vault::Vault::new();
        v.unlock("pulse-vault-pw", tmp.path().to_path_buf())
            .map_err(|e| format!("{e}"))?;

        // Register a manifest entry
        let entry = vault::ManifestEntry {
            id: uuid::Uuid::new_v4(),
            path: "Personal/pulse-test.idea".into(),
            title: Some("Pulse Test".into()),
            extended_type: Some("text".into()),
            creator: "pulse-demo".into(),
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            collective_id: None,
            header_cache: None,
        };
        v.register_idea(entry).map_err(|e| format!("{e}"))?;
        let count = v.idea_count().map_err(|e| format!("{e}"))?;
        let is_unlocked = v.is_unlocked();
        v.lock().map_err(|e| format!("{e}"))?;

        Ok(serde_json::json!({
            "is_locked": !v.is_unlocked(),
            "entry_count": count,
            "was_unlocked": is_unlocked,
        }))
    }));

    // 6. Hall — write .idea to temp dir, read back
    results.push(run_step("Hall", "H", || {
        let tmp = tempfile::tempdir().map_err(|e| format!("{e}"))?;
        let idea_path = tmp.path().join("pulse-test.idea");

        // Create a digit + header + package
        let digit = ideas::Digit::new(
            "text".into(),
            x::Value::from("Hall proof of life"),
            "pulse-demo".into(),
        )
        .map_err(|e| format!("{e}"))?;

        let header = ideas::Header::create(
            "pulse-demo-key".into(),
            "pulse-demo-sig".into(),
            digit.id(),
            ideas::header::KeySlot::Internal(ideas::header::InternalKeySlot {
                key_id: "pulse".into(),
                wrapped_key: "demo".into(),
            }),
        );

        let package = ideas::IdeaPackage::new(idea_path.clone(), header, digit);

        // Generate a content key for encryption
        let key = [0x42u8; 32]; // deterministic key for demo
        let bytes_written = hall::scribe::write(&package, &key, None).map_err(|e| format!("{e}"))?;

        // Read it back
        let read_result = hall::scholar::read(&idea_path, &key, None).map_err(|e| format!("{e}"))?;

        Ok(serde_json::json!({
            "wrote_ok": bytes_written > 0,
            "read_ok": !read_result.value.digits.is_empty(),
            "path": idea_path.display().to_string(),
        }))
    }));

    // 7. Globe — create OmniEvent, sign it
    results.push(run_step("Globe", "G", || {
        let keypair = crown::CrownKeypair::generate();
        let event = globe::EventBuilder::text_note("Pulse heartbeat", &keypair)
            .map_err(|e| format!("{e}"))?;
        let valid = globe::EventBuilder::verify(&event).map_err(|e| format!("{e}"))?;

        Ok(serde_json::json!({
            "event_id": &event.id[..16],
            "kind": event.kind,
            "signature_valid": valid,
        }))
    }));

    // 8. Lingo — Babel encode/decode
    results.push(run_step("Lingo", "L", || {
        let seed = [0x55u8; 32]; // deterministic seed for demo
        let babel = lingo::Babel::new(&seed);
        let original = "hello world";
        let encoded = babel.encode(original);
        let decoded = babel.decode(&encoded);

        Ok(serde_json::json!({
            "original": original,
            "encoded": encoded,
            "decoded_matches": decoded == original,
        }))
    }));

    // 9. Equipment — Phone call
    results.push(run_step("Equipment", "E", || {
        let phone = equipment::Phone::new();
        phone.register_raw("pulse.ping", |_data| Ok(b"pong".to_vec()));

        let response = phone
            .call_raw("pulse.ping", b"ping")
            .map_err(|e| format!("{e}"))?;

        Ok(serde_json::json!({
            "call_id": "pulse.ping",
            "response": String::from_utf8_lossy(&response),
        }))
    }));

    // 10. Polity — constitutional review
    results.push(run_step("Polity", "P", || {
        let rights = polity::RightsRegistry::default();
        let protections = polity::ProtectionsRegistry::default();
        let reviewer = polity::ConstitutionalReviewer::new(&rights, &protections);

        let clean_action = polity::ActionDescription {
            description: "Community votes on garden design".into(),
            actor: "garden-collective".into(),
            violates: vec![],
        };
        let review = reviewer.review(&clean_action);

        Ok(serde_json::json!({
            "review_result": format!("{:?}", review.result),
            "rights_count": review.rights_checked,
            "protections_count": review.protections_checked,
        }))
    }));

    // 11. Kingdom — community with charter
    results.push(run_step("Kingdom", "K", || {
        let community = kingdom::Community::new(
            "Pulse Community",
            kingdom::CommunityBasis::Interest,
        );
        let charter = kingdom::Charter::new(
            community.id,
            "Pulse Charter",
            "Testing the Omnidea stack",
        );

        Ok(serde_json::json!({
            "community_id": community.id.to_string(),
            "charter_version": charter.version,
        }))
    }));

    // 12. Fortune — treasury + mint + UBI
    results.push(run_step("Fortune", "F", || {
        let policy = fortune::FortunePolicy::default_policy();
        let mut treasury = fortune::Treasury::new(policy.clone());
        treasury.update_metrics(fortune::NetworkMetrics {
            active_users: 100,
            total_ideas: 50,
            total_collectives: 2,
        });

        let minted = treasury
            .mint(100, "pulse-recipient", fortune::MintReason::Initial)
            .map_err(|e| format!("{e}"))?;

        let mut ubi = fortune::UbiDistributor::new();
        ubi.verify_identity("pulse-recipient");
        let ledger = fortune::Ledger::new();
        let eligibility = ubi.check_eligibility("pulse-recipient", &ledger, &treasury, &policy);

        Ok(serde_json::json!({
            "treasury_balance": minted,
            "ubi_eligible": eligibility.eligible,
        }))
    }));

    // 13. Bulwark — trust layer
    results.push(run_step("Bulwark", "B", || {
        let bond = bulwark::VisibleBond::new(
            "alice-pubkey",
            "bob-pubkey",
            bulwark::BondDepth::Friend,
        );

        Ok(serde_json::json!({
            "trust_layer": format!("{:?}", bond.depth_from_a),
            "bond_depth": format!("{:?}", bond.effective_depth()),
        }))
    }));

    // 14. Jail — trust graph
    results.push(run_step("Jail", "J", || {
        let mut graph = jail::TrustGraph::new();

        let edge1 = jail::VerificationEdge::new(
            "alice",
            "bob",
            "mutual_vouch",
            jail::VerificationSentiment::Positive,
            0.9,
        );
        let edge2 = jail::VerificationEdge::new(
            "bob",
            "carol",
            "proximity",
            jail::VerificationSentiment::Positive,
            0.8,
        );
        graph.add_edge(edge1).map_err(|e| format!("{e}"))?;
        graph.add_edge(edge2).map_err(|e| format!("{e}"))?;

        let _verifications = jail::trust_graph::query::query_verifications(
            &graph, "alice", "carol", 3,
        );
        let recommendation = jail::trust_graph::recommendation::generate_recommendation(
            jail::trust_graph::pattern::VerificationPattern::Healthy,
            Some(2),
            0,
        );

        Ok(serde_json::json!({
            "nodes": graph.node_count(),
            "edges": graph.edge_count(),
            "recommendation": format!("{recommendation:?}"),
        }))
    }));

    // 15. Regalia — theme + crest + crown_jewels
    results.push(run_step("Regalia", "R", || {
        let reign = regalia::Reign::default();
        let crest = reign.crest();

        // Crown jewels: material cascade
        let style = regalia::FacetStyle::regular();
        let mut sheet = regalia::Stylesheet::new(regalia::FacetStyle::regular());
        sheet.set_override(
            regalia::CrownRole::panel(),
            regalia::FacetStyle::frosted(),
        );
        let resolved = sheet.style_for(&regalia::CrownRole::panel());
        let cascade_works = resolved.frost > style.frost; // frosted > regular

        // SDF math (individual f64 args: px, py, half_w, half_h, radii, smoothing)
        let radii = regalia::CornerRadii::uniform(8.0);
        let sdf_val = regalia::crown_jewels::sdf_rounded_rect(0.0, 0.0, 50.0, 25.0, &radii, 0.0);

        // Color math
        let lum = regalia::crown_jewels::relative_luminance(0.2, 0.4, 0.8);

        Ok(serde_json::json!({
            "theme_name": reign.name,
            "aspect": reign.aspect.name(),
            "crest_primary": format!("{:?}", crest.primary),
            "facet_kind": regalia::FacetStyle::kind(),
            "sdf_at_center": format!("{sdf_val:.2}"),
            "luminance": format!("{lum:.4}"),
            "cascade_works": cascade_works,
        }))
    }));

    // 16. Magic — document state + actions + code builder
    results.push(run_step("Magic", "M", || {
        // Create document + insert a digit
        let mut state = magic::DocumentState::new("pulse-demo");
        let digit = ideas::Digit::new(
            "text".into(),
            x::Value::from("Hello from Pulse"),
            "pulse-demo".into(),
        )
        .map_err(|e| format!("{e}"))?;
        let digit_id = digit.id();
        let _op = state.insert_digit(digit, None).map_err(|e| format!("{e}"))?;
        let has_digit = state.digit(digit_id).is_some();

        // Action execute + undo/redo tracking
        let action = magic::Action::delete(digit_id);
        let (op, inverse) = action.execute(&mut state).map_err(|e| format!("{e}"))?;
        let mut history = magic::DocumentHistory::new();
        history.record(magic::HistoryEntry {
            operation: op,
            inverse,
        });
        let can_undo = history.can_undo();
        let can_redo = history.can_redo();

        // Code builder
        let mut builder = magic::CodeBuilder::new();
        builder
            .line("import SwiftUI")
            .blank()
            .braced("struct PulseView: View", |b| {
                b.braced("var body: some View", |b| {
                    b.line("Text(\"Omnidea lives\")");
                });
            });
        let code = builder.output();

        Ok(serde_json::json!({
            "digit_inserted": has_digit,
            "can_undo": can_undo,
            "can_redo": can_redo,
            "code_lines": code.lines().count(),
        }))
    }));

    // 17. Advisor — thought + session + synapse + pressure
    results.push(run_step("Advisor", "A", || {
        // Session + thought
        let home = advisor::Session::home();
        let thought = advisor::Thought::new(
            home.id,
            "Omnidea lives — the mind layer is complete",
            advisor::ThoughtSource::Autonomous,
        )
        .with_priority(advisor::ThoughtPriority::High);

        // Synapse
        let synapse = advisor::Synapse::session_contains_thought(home.id, thought.id, 0.8);

        // Expression pressure
        let mut pressure = advisor::ExpressionPressure::new();
        pressure.apply(
            &advisor::PressureEvent::NovelContent,
            &advisor::PressureConfig::default(),
        );
        let snapshot = pressure.snapshot(0.6, 0.9);

        // Cognitive store
        let mut store = advisor::CognitiveStore::new(100);
        store.save_session(home.clone());
        store.save_thought(thought.clone());
        store.save_synapse(synapse.clone());

        Ok(serde_json::json!({
            "session_type": format!("{:?}", home.session_type),
            "thought_priority": format!("{:?}", thought.priority),
            "synapse_strength": synapse.strength,
            "pressure_level": snapshot.level(),
            "store_thoughts": store.thoughts_for_session(home.id).len(),
        }))
    }));

    let passed = results.iter().filter(|r| r.success).count();
    let report = PulseReport {
        version: "2.0".into(),
        crates_tested: results.len(),
        crates_passed: passed,
        results,
    };

    json_to_c(&report)
}
