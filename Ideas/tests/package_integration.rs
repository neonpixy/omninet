use ideas::authority::{Book, Root, Relationship, Tree};
use ideas::bonds::{BondRelationship, Bonds, LocalBondReference, LocalBonds, PublicBondReference, PublicBonds};
use ideas::coinage::{Cool, Provider, RedeemableType, Redemption, Terms};
use ideas::digit::Digit;
use ideas::header::{Header, KeySlot, PasswordKeySlot};
use ideas::package::IdeaPackage;
use ideas::position::{Coordinates, Position};
use tempfile::tempdir;
use uuid::Uuid;
use x::Value;

fn test_key_slot() -> KeySlot {
    KeySlot::Password(PasswordKeySlot {
        salt: "dGVzdHNhbHQ=".into(),
        nonce: "dGVzdG5vbmNl".into(),
        wrapped_key: "d3JhcHBlZA==".into(),
    })
}

#[test]
fn create_save_load_full_package() {
    let dir = tempdir().unwrap();
    let idea_path = dir.path().join("test-doc.idea");

    // Create a root digit
    let root = Digit::new("text".into(), Value::from("Hello, Omnidea!"), "cpub1alice".into())
        .unwrap()
        .with_property("font".into(), Value::from("system"), "cpub1alice")
        .with_property("size".into(), Value::Double(16.0), "cpub1alice");

    // Create a child digit
    let child = Digit::new(
        "image".into(),
        Value::Data(vec![0xFF, 0xD8, 0xFF]),
        "cpub1alice".into(),
    )
    .unwrap();

    let root = root.with_child(child.id(), "cpub1alice");

    // Create header
    let header = Header::create(
        "cpub1alice".into(),
        "sig_placeholder".into(),
        root.id(),
        test_key_slot(),
    );

    // Build the full package
    let package = IdeaPackage::new(idea_path.clone(), header, root)
        .with_digit(child)
        .with_book(Book::new("cpub1alice".into(), "sig_book".into()))
        .with_tree(
            Tree::new().with_root(Root {
                idea_id: Uuid::new_v4(),
                creator: "cpub1bob".into(),
                relationship: Relationship::Inspiration,
                contribution_weight: 20,
                timestamp: chrono::Utc::now(),
                signature: "sig_root".into(),
            }),
        )
        .with_cool(Cool::new(100_000))
        .with_bonds(Bonds {
            local: Some(LocalBonds {
                references: vec![LocalBondReference {
                    idea_id: Uuid::new_v4(),
                    path: "/Users/test/other.idea".into(),
                    relationship: BondRelationship::Related,
                    verified: false,
                    last_verified: None,
                }],
            }),
            private_bonds: None,
            public_bonds: Some(PublicBonds {
                references: vec![PublicBondReference {
                    crown_id: "note1abc".into(),
                    idea_id: Uuid::new_v4(),
                    creator: "cpub1charlie".into(),
                    relationship: BondRelationship::Cites,
                    relays: vec!["wss://relay.damus.io".into()],
                    verified: false,
                    last_fetched: None,
                    cached: false,
                }],
            }),
        })
        .with_position(Position::new(
            Coordinates {
                x: 42.0,
                y: -17.5,
                z: 0.0,
            },
            true,
        ));

    // Save to disk
    package.save().unwrap();

    // Verify directory structure
    assert!(idea_path.join("Header.json").exists());
    assert!(idea_path.join("Content").is_dir());
    assert!(idea_path.join("Authority").is_dir());
    assert!(idea_path.join("Authority/book.json").exists());
    assert!(idea_path.join("Authority/tree.json").exists());
    assert!(idea_path.join("Bonds").is_dir());
    assert!(idea_path.join("Bonds/local.json").exists());
    assert!(idea_path.join("Bonds/public.json").exists());
    assert!(idea_path.join("Coinage").is_dir());
    assert!(idea_path.join("Coinage/value.json").exists());
    assert!(idea_path.join("Position").is_dir());
    assert!(idea_path.join("Position/position.json").exists());

    // Load it back
    let loaded = IdeaPackage::load(&idea_path).unwrap();

    // Verify header
    assert_eq!(loaded.header.id, package.header.id);
    assert_eq!(loaded.header.version, "1.0");
    assert_eq!(loaded.header.creator.public_key, "cpub1alice");

    // Verify digits
    assert_eq!(loaded.digits.len(), 2); // root + child
    let loaded_root = loaded.root_digit().unwrap();
    assert_eq!(loaded_root.digit_type(), "text");
    assert_eq!(loaded_root.content, Value::from("Hello, Omnidea!"));
    assert!(loaded_root.has_children());

    // Verify authority
    assert!(loaded.book.is_some());
    assert!(loaded.book.as_ref().unwrap().is_owner("cpub1alice"));
    assert!(loaded.tree.is_some());
    assert_eq!(loaded.tree.as_ref().unwrap().roots.len(), 1);
    assert_eq!(
        loaded.tree.as_ref().unwrap().roots[0].contribution_weight,
        20
    );

    // Verify coinage
    assert!(loaded.cool.is_some());
    assert_eq!(loaded.cool.as_ref().unwrap().cool, 100_000);

    // Verify bonds
    assert!(loaded.bonds.is_some());
    let bonds = loaded.bonds.as_ref().unwrap();
    assert_eq!(bonds.count(), 2); // 1 local + 1 public

    // Verify position
    assert!(loaded.position.is_some());
    let pos = loaded.position.as_ref().unwrap();
    assert_eq!(pos.coordinates.x, 42.0);
    assert!(pos.pinned);
}

#[test]
fn minimal_package_round_trip() {
    let dir = tempdir().unwrap();
    let idea_path = dir.path().join("minimal.idea");

    let root = Digit::new("text".into(), Value::from("minimal"), "cpub1test".into()).unwrap();
    let header = Header::create(
        "cpub1test".into(),
        "sig".into(),
        root.id(),
        test_key_slot(),
    );

    let package = IdeaPackage::new(idea_path.clone(), header, root);
    package.save().unwrap();

    let loaded = IdeaPackage::load(&idea_path).unwrap();
    assert_eq!(loaded.digits.len(), 1);
    assert!(loaded.book.is_none());
    assert!(loaded.tree.is_none());
    assert!(loaded.cool.is_none());
    assert!(loaded.bonds.is_none());
    assert!(loaded.position.is_none());
}

#[test]
fn read_header_only() {
    let dir = tempdir().unwrap();
    let idea_path = dir.path().join("header-only.idea");

    let root = Digit::new("text".into(), Value::Null, "cpub1test".into()).unwrap();
    let header = Header::create(
        "cpub1test".into(),
        "sig".into(),
        root.id(),
        test_key_slot(),
    );
    let package = IdeaPackage::new(idea_path.clone(), header, root);
    package.save().unwrap();

    // Read just the header (no loading digits or other components)
    let header = IdeaPackage::read_header(&idea_path).unwrap();
    assert_eq!(header.version, "1.0");
    assert_eq!(header.creator.public_key, "cpub1test");
}

#[test]
fn load_nonexistent_directory() {
    let result = IdeaPackage::load(std::path::Path::new("/nonexistent/path.idea"));
    assert!(result.is_err());
}

#[test]
fn redemption_package() {
    let dir = tempdir().unwrap();
    let idea_path = dir.path().join("service.idea");

    let root = Digit::new(
        "service".into(),
        Value::from("Haircut at Bob's"),
        "cpub1bob".into(),
    )
    .unwrap();
    let header = Header::create(
        "cpub1bob".into(),
        "sig".into(),
        root.id(),
        test_key_slot(),
    );

    let package = IdeaPackage::new(idea_path.clone(), header, root)
        .with_cool(Cool::new(Cool::SIMPLE_SERVICE))
        .with_redemption(Redemption::new(
            RedeemableType::Service,
            Provider {
                public_key: "cpub1bob".into(),
                name: "Bob's Barbershop".into(),
                contact: Some("bob@barber.cool".into()),
            },
            Terms {
                description: "One standard haircut".into(),
                location: Some("123 Main St".into()),
                valid_until: None,
                conditions: vec!["Appointment required".into()],
            },
        ));

    package.save().unwrap();

    let loaded = IdeaPackage::load(&idea_path).unwrap();
    assert!(loaded.redemption.is_some());
    let r = loaded.redemption.as_ref().unwrap();
    assert!(r.can_redeem());
    assert_eq!(r.provider.name, "Bob's Barbershop");

    assert!(loaded.cool.is_some());
    assert_eq!(loaded.cool.as_ref().unwrap().cool, Cool::SIMPLE_SERVICE);
}
