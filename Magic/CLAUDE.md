# Magic — Rendering & Code Projection

The arcane arts. Magic has three faces: Ideation holds the truth, Imagination renders it visually, and Projection outputs it as live working code. Design something and the code already exists — because both are projections of the same .idea.

## Architecture: Triple-I + Projection

### Triple-I (from NeonPixy)

- **Ideation** — Single source of truth. Owns the DocumentState: Digits (from Ideas crate), selection, layout, vector clock. All mutations go through `apply(DigitOperation)`. No other path to state change.
- **Imagination** — Visual rendering. DigitRenderer trait maps Digit types to platform views. Registry pattern with FallbackRenderer (unknown types are rendered, never lost). Platform-specific — SwiftUI on Apple, HTML on web, etc.
- **Initiation** — Action dispatch. User actions become invertible Actions → DigitOperations. DocumentHistory provides undo/redo (max depth 100). Operations broadcast via CRDT for collaboration.

### Projection (evolved from Swiftlight's export pipeline)

**Design = Code.** Projection isn't an "export" step — it's a live, continuous output of working frontend code derived from the same Ideation state that Imagination renders visually.

```
Ideation (Digits) ──┬── Imagination ──→ Visual (SwiftUI views on screen)
                    └── Projection ───→ Code (SwiftUI / React / Flutter / HTML)
```

Both update on every state change. Change the design, both the visual and the code update simultaneously. Switch code targets with a dropdown.

**Why this works better than Swiftlight's export:** Swiftlight tried visual → infer → generate (lossy). Magic does explicit data → project (lossless for the data you have). Digits explicitly declare layout, spacing, alignment, tokens — no guessing from pixel positions.

**Regalia layout bridge — the key insight:** Regalia's Sanctum-based layout system maps nearly 1:1 to platform layout primitives. Projection reads Sanctum declarations directly — no pixel-to-layout inference needed. This is what makes design = code actually work:

| Regalia Formation | SwiftUI | CSS/HTML | Flutter |
|---|---|---|---|
| Rank | HStack | flex-row | Row |
| Column | VStack | flex-column | Column |
| Tier | ZStack | position: absolute (stacked) | Stack |
| Procession | LazyVGrid / flow | flex-wrap | Wrap |
| OpenCourt | Canvas / GeometryReader | position: absolute | CustomMultiChildLayout |

Projection reads FormationKind from digit properties or `.excalibur` declarations. Sanctum nesting maps to view nesting. Appointments (resolved frames) provide fallback absolute positions.

## Key Types

### Ideation (`ideation/`)
- **DocumentState** — Single source of truth. Owns digits HashMap, CrdtEngine for idempotency, VectorClock, SelectionState (transient, NOT CRDT), DocumentLayout. All mutations through `apply(DigitOperation)` which returns `Ok(true)` if applied, `Ok(false)` if duplicate. Convenience methods: `insert_digit()`, `update_digit()`, `delete_digit()`, `load_digits()`.
- **SelectionState** — Transient UI state: selected digit IDs (HashSet), focused digit, text selection. Local only, never broadcast.
- **DocumentLayout** — Layout mode (Vertical/Horizontal/Grid/Freeform), page dimensions, margins, optional grid snap. Defaults to A4-ish (595x842).
- **DigitTypeRegistry** — Maps type strings to DigitTypeDefinition. 9 core types pre-registered via `with_core_types()`: text, code, image, embed, document, container, table, divider, link. `DigitCategory` enum.

### Initiation (`initiation/`)
- **Action** — Enum with 5 variants: InsertDigit, UpdateDigit, DeleteDigit, MoveDigit, TransformDigit. `execute(state)` returns `(DigitOperation, Action)` — the applied operation + pre-computed inverse for undo. Inverse captures "before" state (e.g., DeleteDigit snapshots the digit before tombstoning, UpdateDigit swaps old/new values).
- **DocumentHistory** — Undo/redo stacks of HistoryEntry (operation + inverse action). Max depth 100. Record clears redo stack.
- **ActionRegistry** — `ActionHandler` trait for custom action handlers.

### Imagination (`imagination/`)
- **DigitRenderer** trait — `digit_type()`, `supported_modes()`, `render(digit, mode, context)` → RenderSpec, `estimated_size()`. Send + Sync. Object-safe.
- **RenderSpec** — Platform-agnostic render description: digit_id, type, mode, estimated size, properties HashMap. Produced by DigitRenderer implementations.
- **RenderMode** — Display / Editing / Thumbnail / Print.
- **RenderContext** — Environmental: available width/height, ColorScheme (Light/Dark), text scale, reduce motion.
- **RendererRegistry** — Maps digit types to DigitRenderer implementations. FallbackRenderer handles unknown types (nothing is ever invisible).
- **RenderCache** — LRU cache keyed by (digit_id, RenderMode). Max 200 entries. Hit/miss tracking.

### Projection (`projection/`)
- **CodeProjection** trait — `name()` (e.g., "SwiftUI"), `file_extension()` (e.g., "swift"), `project(context) -> Vec<GeneratedFile>`. Send + Sync. Each implementation is a plugin.
- **GeneratedFile** — relative_path + FileContents (Text or Binary).
- **ProjectionContext** — Pre-computed in one O(n) pass via `build()`: digit_index, children_index, root_ids, formation_map (digit → FormationKind from properties), Reign for token resolution, resolved Appointments. `with_sanctum_formations()` attaches Sanctum declarations. `crest()` resolves from Reign.
- **CodeBuilder** — Indent-aware string builder. `line()`, `blank()`, `comment()`, `braced()`, `indent()`. Produces clean formatted source for any language.
- **NameResolver** — Case transforms (PascalCase, camelCase, kebab-case, SCREAMING_SNAKE) with deduplication tracking and reserved word escaping.

### Error (`error.rs`)
- **MagicError** — 8 variants: DigitNotFound(Uuid), UnregisteredType, RendererNotFound, ActionFailed, HistoryEmpty, InvalidOperation, ProjectionError, Serialization. Implements `From<serde_json::Error>`.

## Dependencies

```toml
x = { path = "../X" }         # Value, CrdtEngine, VectorClock
ideas = { path = "../Ideas" }  # Digit, DigitOperation, CRDT types
regalia = { path = "../Regalia" } # Sanctum, FormationKind, Reign, Appointment, Crest
serde, serde_json, thiserror, uuid, chrono, log
```

**Zero async.** Magic is pure data structures and logic.

## New Luminaria Vision

Magic is the engine behind the merged Swiftlight + Luminaria — **New Luminaria** (a Throne lens):
- **Luminaria's morphing glass shell** (circle ↔ rectangle, the app wears its own design system)
- **Swiftlight's canvas + 17 tools** (design tool capabilities)
- **Magic's Ideation** (universal document model via .idea)
- **Magic's Projection** (live code output in any target)
- **Regalia's tokens** (semantic design language)
- **Divinity's materials** (glass rendering via CrystalKit on Apple, Vulkan/WebGPU on other platforms)
- **Advisor's AI** (AI companion that understands and modifies designs)

This is Phase 6 (Throne). Phase 5 builds the Rust foundations.

## Covenant Alignment

**Sovereignty** — code is transparent and editable; you own what you design. **Dignity** — every content type gets rendered, even unknown ones (FallbackRenderer). **Consent** — renderers and projections are registered plugins; the system is open to extension.
