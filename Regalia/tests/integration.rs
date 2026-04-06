use std::collections::HashMap;

use regalia::*;

#[test]
fn design_token_lifecycle() {
    // Create a theme
    let reign = Reign::default();

    // Resolve colors for light mode
    let crest = reign.crest();
    assert_eq!(crest.primary, Ember::BLACK);
    assert_eq!(crest.background, Ember::WHITE);

    // Switch to dark mode
    let dark = Reign::new("Dark", Aura::default(), Aspect::dark());
    let dark_crest = dark.crest();
    assert_eq!(dark_crest.primary, Ember::WHITE);
    assert_eq!(dark_crest.background, Ember::BLACK);

    // All token scales resolve
    assert_eq!(reign.span().md, 16.0);
    assert_eq!(reign.inscription().body.size, 16.0);
    assert_eq!(reign.arch().md, 12.0);
}

#[test]
fn layout_engine_sidebar_toolbar_content() {
    let sanctums = vec![
        Sanctum::sidebar(Some(200.0), None),
        Sanctum::toolbar(Some(44.0), None),
        Sanctum::content(None),
    ];

    let widget = MockClansman::named("main-content", Some((100.0, 100.0)));
    let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
        SanctumID::content(),
        vec![&widget as &dyn Clansman],
    )]);

    let domain = Arbiter::resolve(
        (0.0, 0.0, 1024.0, 768.0),
        &sanctums,
        &vassals,
        &HashMap::new(),
        None,
    )
    .unwrap();

    // Sidebar takes left 200px
    let sb = domain.sanctum_bounds_for(&SanctumID::sidebar()).unwrap();
    assert_eq!(sb.2, 200.0);

    // Toolbar takes top 44px (after sidebar carve)
    let tb = domain.sanctum_bounds_for(&SanctumID::toolbar()).unwrap();
    assert_eq!(tb.3, 44.0);

    // Content gets the remaining space
    let content = domain.sanctum_bounds_for(&SanctumID::content()).unwrap();
    assert_eq!(content.0, 200.0); // after sidebar
    assert_eq!(content.1, 44.0); // after toolbar
    assert_eq!(content.2, 824.0); // 1024 - 200
    assert_eq!(content.3, 724.0); // 768 - 44

    // Widget is placed in content area
    assert_eq!(domain.appointments.len(), 1);
    assert!(domain.appointments[0].x >= 200.0);
    assert!(domain.appointments[0].y >= 44.0);
}

#[test]
fn formation_rank_places_horizontally() {
    let sanctums = vec![Sanctum {
        id: SanctumID::content(),
        border: None,
        fixed_extent: None,
        seat: Seat::Center,
        z_layer: 0,
        clips: true,
        formation_kind: FormationKind::Rank {
            spacing: 10.0,
            alignment: RankAlignment::Center,
            justification: RankJustification::Leading,
        },
        subsanctums: vec![],
    }];

    let c1 = MockClansman::named("btn1", Some((80.0, 40.0)));
    let c2 = MockClansman::named("btn2", Some((80.0, 40.0)));
    let c3 = MockClansman::named("btn3", Some((80.0, 40.0)));
    let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
        SanctumID::content(),
        vec![&c1 as &dyn Clansman, &c2 as &dyn Clansman, &c3 as &dyn Clansman],
    )]);

    let domain = Arbiter::resolve(
        (0.0, 0.0, 400.0, 100.0),
        &sanctums,
        &vassals,
        &HashMap::new(),
        None,
    )
    .unwrap();

    assert_eq!(domain.appointments.len(), 3);
    // Each button after the previous, with spacing
    assert!(domain.appointments[1].x > domain.appointments[0].x);
    assert!(domain.appointments[2].x > domain.appointments[1].x);
}

#[test]
fn formation_column_places_vertically() {
    let sanctums = vec![Sanctum {
        id: SanctumID::content(),
        border: None,
        fixed_extent: None,
        seat: Seat::Center,
        z_layer: 0,
        clips: true,
        formation_kind: FormationKind::Column {
            spacing: 8.0,
            alignment: ColumnAlignment::Leading,
            justification: ColumnJustification::Top,
        },
        subsanctums: vec![],
    }];

    let c1 = MockClansman::named("row1", Some((200.0, 30.0)));
    let c2 = MockClansman::named("row2", Some((200.0, 30.0)));
    let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
        SanctumID::content(),
        vec![&c1 as &dyn Clansman, &c2 as &dyn Clansman],
    )]);

    let domain = Arbiter::resolve(
        (0.0, 0.0, 400.0, 300.0),
        &sanctums,
        &vassals,
        &HashMap::new(),
        None,
    )
    .unwrap();

    assert_eq!(domain.appointments.len(), 2);
    assert!(domain.appointments[1].y > domain.appointments[0].y);
}

#[test]
fn surge_animation_curves() {
    // Spring overshoots
    let spring = SpringSurge::new(0.3, 3.5);
    let max = (0..200)
        .map(|i| spring.value(i as f64 * 0.01))
        .fold(0.0_f64, f64::max);
    assert!(max > 1.0);

    // Ease is monotonic and bounded
    let ease = EaseSurge::default();
    assert_eq!(ease.value(0.0), 0.0);
    assert_eq!(ease.value(ease.duration()), 1.0);

    // Linear is proportional
    let linear = LinearSurge::new(1.0);
    assert!((linear.value(0.5) - 0.5).abs() < f64::EPSILON);

    // Decay approaches 1
    let decay = DecaySurge::default();
    assert!(decay.value(3.0) > 0.99);

    // Snap is instant
    let snap = SnapSurge;
    assert_eq!(snap.value(0.0), 1.0);
    assert!(snap.is_complete(0.0, 0.0));
}

#[test]
fn theme_serialization_roundtrip() {
    let mut aura = Aura::default();
    aura.dark_crest
        .custom
        .insert("brand".into(), Ember::from_hex("#FF6600").unwrap());
    aura.span
        .custom
        .insert("page-margin".into(), 64.0);

    let reign = Reign::new("My Theme", aura, Aspect::dark());
    let json = serde_json::to_string_pretty(&reign).unwrap();

    let decoded: Reign = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.name, "My Theme");
    assert_eq!(decoded.aspect, Aspect::dark());
    assert_eq!(
        decoded.crest().get_custom("brand").unwrap().to_hex(),
        "#FF6600"
    );
    assert_eq!(decoded.span().get_custom("page-margin"), Some(64.0));
}
