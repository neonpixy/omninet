use ideas::Digit;
use magic::{
    Action, CodeBuilder, DocumentHistory, DocumentState, HistoryEntry, NameResolver,
    ProjectionContext, RenderCache, RenderContext, RenderMode, RenderSpec, RendererRegistry,
};
use regalia::{FormationKind, Reign};
use x::Value;

fn make_text(text: &str) -> Digit {
    Digit::new("text".into(), Value::from(text), "cpub1test".into()).unwrap()
}

fn make_container() -> Digit {
    Digit::new("container".into(), Value::Null, "cpub1test".into()).unwrap()
}

#[test]
fn document_lifecycle() {
    let mut state = DocumentState::new("cpub1alice");

    // Insert root container
    let container = make_container();
    let cid = container.id();
    state.insert_digit(container, None).unwrap();
    assert_eq!(state.root_digit_id(), Some(cid));

    // Insert children
    let a = make_text("hello");
    let b = make_text("world");
    let aid = a.id();
    let bid = b.id();
    state.insert_digit(a, Some(cid)).unwrap();
    state.insert_digit(b, Some(cid)).unwrap();
    assert_eq!(state.digit_count(), 3);
    assert_eq!(state.children_of(cid).len(), 2);

    // Update
    state
        .update_digit(aid, "content".into(), Value::from("hello"), Value::from("hi"))
        .unwrap();
    assert_eq!(state.digit(aid).unwrap().content, Value::from("hi"));

    // Delete
    state.delete_digit(bid).unwrap();
    assert!(state.digit(bid).unwrap().is_deleted());
}

#[test]
fn action_undo_redo() {
    let mut state = DocumentState::new("cpub1alice");
    let mut history = DocumentHistory::new();

    // Insert
    let digit = make_text("hello");
    let id = digit.id();
    let (op, inverse) = Action::insert(digit, None).execute(&mut state).unwrap();
    history.record(HistoryEntry {
        operation: op,
        inverse,
    });
    assert_eq!(state.digit_count(), 1);

    // Undo (execute the inverse)
    let entry = history.pop_undo().unwrap();
    let (redo_op, redo_inverse) = entry.inverse.execute(&mut state).unwrap();
    history.push_redo(HistoryEntry {
        operation: redo_op,
        inverse: redo_inverse,
    });
    // Digit should be tombstoned
    assert!(state.digit(id).unwrap().is_deleted());

    // Redo (execute the redo inverse, which re-inserts)
    let redo_entry = history.pop_redo().unwrap();
    redo_entry.inverse.execute(&mut state).unwrap();
    // The original digit snapshot overwrites the tombstoned version
    assert_eq!(state.digit_count(), 1);
    assert!(!state.digit(id).unwrap().is_deleted());
}

#[test]
fn renderer_registry_with_fallback() {
    let reg = RendererRegistry::new();
    let digit = make_text("test");
    let ctx = RenderContext::default();

    // Should use fallback for unregistered type
    let spec = reg.render(&digit, RenderMode::Display, &ctx);
    assert_eq!(spec.digit_type, "text");
    assert_eq!(
        spec.properties.get("fallback"),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn render_cache_lifecycle() {
    let mut cache = RenderCache::new();
    let id = uuid::Uuid::new_v4();

    // Miss
    assert!(cache.get(id, RenderMode::Display).is_none());
    assert_eq!(cache.misses(), 1);

    // Insert
    let spec = RenderSpec::new(id, "text", RenderMode::Display).with_size(200.0, 30.0);
    cache.insert(spec);

    // Hit
    let cached = cache.get(id, RenderMode::Display).unwrap();
    assert_eq!(cached.estimated_width, 200.0);
    assert_eq!(cache.hits(), 1);
    assert!((cache.hit_rate() - 0.5).abs() < f64::EPSILON);

    // Invalidate
    cache.invalidate(id);
    assert_eq!(cache.size(), 0);
}

#[test]
fn code_builder_swiftui_output() {
    let mut b = CodeBuilder::new();
    b.braced("struct ContentView: View", |b| {
        b.braced("var body: some View", |b| {
            b.braced("HStack(spacing: 8)", |b| {
                b.line("Text(\"Hello\")");
                b.line("Text(\"World\")");
            });
        });
    });
    let output = b.output();
    assert!(output.contains("HStack(spacing: 8)"));
    assert!(output.contains("Text(\"Hello\")"));
    // Proper indentation
    assert!(output.contains("        Text(\"Hello\")"));
}

#[test]
fn projection_context_from_digits() {
    let parent = make_container()
        .with_property("formation".into(), Value::from("rank"), "cpub1test");
    let child_a = make_text("a");
    let child_b = make_text("b");
    let parent_with_children = parent
        .with_child(child_a.id(), "cpub1test")
        .with_child(child_b.id(), "cpub1test");
    let pid = parent_with_children.id();

    let ctx = ProjectionContext::build(
        &[parent_with_children, child_a.clone(), child_b.clone()],
        Some(pid),
        Reign::default(),
    );

    assert_eq!(ctx.root_ids, vec![pid]);
    assert_eq!(ctx.children_of(pid).len(), 2);
    assert!(ctx.digit(child_a.id()).is_some());

    // Formation should be Rank (from "rank" property)
    let formation = ctx.formation_for(pid).unwrap();
    assert!(matches!(formation, FormationKind::Rank { .. }));

    // Name resolution
    let mut resolver = NameResolver::new();
    assert_eq!(resolver.type_name("content-view"), "ContentView");
    assert_eq!(resolver.type_name("content-view"), "ContentView2");
}
