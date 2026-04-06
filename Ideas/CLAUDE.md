# Ideas — Universal Content Format

Everything is an `.idea`. Documents, songs, drawings, services, physical goods, AI thoughts, live streams — every object in Omninet is an `.idea` package. This is the universal tongue.

An `.idea` is a directory package (like `.app` bundles) containing six subsystems: content (Digits), metadata (Header), ownership (Authority), references (Bonds), value (Coinage), and position (lightweight coordinates).

## Source Layout

```
Ideas/
├── Cargo.toml
├── src/
│   ├── lib.rs            ← module declarations + re-exports
│   ├── error.rs          ← IdeasError (thiserror)
│   ├── validation.rs     ← Regex patterns for types/keys/paths
│   ├── helpers.rs        ← Shared property extraction helpers for domain modules
│   ├── digit.rs          ← Digit (atomic content unit)
│   ├── header.rs         ← Header, Creator, KeySlot, BabelConfig, EncryptionConfig
│   ├── authority.rs      ← Book (ownership) + Tree (provenance)
│   ├── bonds.rs          ← Bonds (local/private/public references) + BondRelationship
│   ├── coinage.rs        ← Cool (currency) + Redemption (fulfillment)
│   ├── position.rs       ← Coordinates + Position (lightweight)
│   ├── crdt.rs           ← DigitOperation (implements x::CrdtOperation)
│   ├── media.rs          ← Media digit helpers (image, audio, video, stream)
│   ├── package.rs        ← IdeaPackage (create/save/load .idea dirs)
│   ├── schema.rs         ← DigitSchema, PropertyType, SchemaRegistry, validation, versioning, composability
│   ├── accessibility.rs  ← Accessibility metadata (a11y_ properties on any digit)
│   ├── sheet.rs          ← Sheet/cell digit helpers (Abacus)
│   ├── slide.rs          ← Slide digit helpers (Podium)
│   ├── form.rs           ← Form element digit helpers (Studio Interactive)
│   ├── richtext.rs       ← Rich text block digit helpers (Quill)
│   ├── interactive.rs    ← Interactive element digit helpers (buttons, nav, tabs)
│   ├── commerce.rs       ← Commerce digit helpers (products, orders, reviews)
│   └── binding.rs        ← Cross-file DataSource bindings
└── tests/
    └── package_integration.rs
```

### .idea Package Format (on disk)
```
MyDocument.idea/
├── Header.json              (cleartext, always)
├── Content/{uuid}.json      (plaintext Phase 1, encrypted Phase 2)
├── Authority/book.json      (optional)
├── Authority/tree.json      (optional)
├── Bonds/local.json         (optional)
├── Bonds/private.json       (optional)
├── Bonds/public.json        (optional)
├── Coinage/value.json       (optional)
├── Coinage/redemption.json  (optional)
├── Position/position.json   (optional)
└── .crdt/operations.log     (hidden)
```

## Key Types

- **Digit** — id (immutable UUID), type (validated regex `[a-z][a-z0-9.-]*`), content (Value), properties (HashMap), children (optional Vec<UUID>), author (immutable crown_id), vector clock, tombstone. Mutations return new instances. Has `accessibility()` convenience method.
- **Header** — version "1.0", id, created/modified, creator (pubkey + sig), content metadata (root digit, count, types), encryption config (AES-256-GCM + key slots), babel config.
- **Book** — creator (immutable), current owner, transfer chain, endorsements.
- **Tree** — roots (parent ideas, contribution weights summing to 100 max), branches, references.
- **Bonds** — local (file paths, must be absolute), private (private relays), public (public relays). BondRelationship enum (Uses, Mentions, Cites, DerivesFrom, RespondsTo, Contradicts, Supports, Related, DataSource).
- **Cool** — value in cool cents, initial value (immutable), valuation history, splits (must sum to 100%).
- **Redemption** — service/physical fulfillment lifecycle.
- **Position** — coordinates (x, y, z) + pinned flag. Complex spatial stuff deferred.
- **DigitOperation** — implements `x::CrdtOperation`. Insert/Update/Delete/Move/Transform payloads.

### Property Key Validation
Property keys must match `[a-zA-Z][a-zA-Z0-9_-]*` — letters, digits, underscores, and hyphens. Max 64 characters.

### Media System (`media.rs`)
Typed constructors and parsers for media content. Media metadata lives in Digit properties as `Value` types — no structural changes to Digit.

- **ImageMeta** — hash, mime, width, height, size, blurhash, thumbnail_hash, alt. Digit type: `media.image`.
- **AudioMeta** — hash, mime, duration_secs, bitrate, channels, sample_rate, codec. Digit type: `media.audio`.
- **VideoMeta** — chunks (list of content-addressed chunk hashes), mime, width, height, duration_secs, bitrate, codec, thumbnail_hash, blurhash. Digit type: `media.video`.
- **StreamMeta** — title, stream_kind (Music/Talk/Video/Screen), status (Scheduled/Live/Ended), relay_url, session_id, thumbnail_hash, fortune_config. Digit type: `media.stream`.
- **StreamFortuneConfig** — tips_enabled, ticket_price, splits (crown_id, percentage pairs).

Each media type has a constructor (`image_digit()`, etc.) and a parser (`parse_image_meta()`, etc.) that round-trip through standard Digit properties.

### Schema System (`schema.rs`)
Content validation blueprints. Optional — untyped Digits still work.
- `DigitSchema` — defines required/optional properties for a Digit type string
- `PropertyType` — String, Int, Double, Bool, Date, Data, Array, Dict (maps to Value variants)
- `PropertyDef` — type + required flag + optional default value
- `validate(digit, schema) -> Result<(), Vec<ValidationError>>` — checks required fields, type mismatches
- `SchemaRegistry` — register/lookup schemas by Digit type string
- Schema versioning: `brand.logo.v1` -> `brand.logo.v2` with migration hints
- Composable schemas: base + extension (e.g., `brand.asset` base, `brand.logo` extends)
- Schemas are data (Serialize + Deserialize), not code — shareable and discoverable

### Accessibility (`accessibility.rs`)
Cross-cutting accessibility metadata stored with `a11y_` prefix in Digit properties.
- **AccessibilityMetadata** — role, label, value, hint, heading_level, language, focus_order, live_region.
- **AccessibilityRole** — Button, Link, Image, Heading, List, ListItem, TextField, Checkbox, Slider, Tab, Table, Cell, Form, Navigation, Alert, Dialog, Custom(String).
- **LiveRegion** — Polite, Assertive, Off.
- `with_accessibility(digit, meta, author) -> Digit` — attaches a11y metadata.
- `accessibility_metadata(digit) -> Option<AccessibilityMetadata>` — extracts a11y metadata.
- `Digit::accessibility()` — convenience method calling the parser.

### Sheet System (`sheet.rs`)
Spreadsheet/database types for the Abacus program.
- **SheetMeta** — name, columns (Vec<ColumnDef>), default_view. Digit type: `data.sheet`.
- **CellMeta** — address (CellAddress), cell_type, value. Digit type: `data.cell`.
- **CellType** — Text, Number, Date, Boolean, Formula, Reference, Rich.
- **ViewMode** — Grid, Kanban, Calendar, Gallery.
- **CellAddress** — optional sheet, column, row. Supports cross-sheet references (e.g., "Revenue!AA100").
- **CellRange** — start + end CellAddress.
- **ColumnDef** — name, cell_type, required, unique.
- Schema functions: `sheet_schema()`, `cell_schema()`.

### Slide System (`slide.rs`)
Presentation types for the Podium program.
- **SlideMeta** — title, speaker_notes, transition, layout, order. Digit type: `presentation.slide`.
- **TransitionType** — Fade, Slide, Push, Dissolve, Custom(String).
- **SlideLayout** — Title, Content, TwoColumn, Blank, Custom(String).
- Schema function: `slide_schema()`.

### Form System (`form.rs`)
Interactive form elements for Studio Interactive.
- **InputFieldMeta** — input_type, label, placeholder, required, pattern. Digit type: `form.input`.
- **CheckboxMeta** — label, checked. Digit type: `form.checkbox`.
- **RadioMeta** — label, group, value. Digit type: `form.radio`.
- **ToggleMeta** — label, on. Digit type: `form.toggle`.
- **DropdownMeta** — label, options, selected. Digit type: `form.dropdown`.
- **SubmitMeta** — label, action_ref. Digit type: `form.submit`.
- **FormMeta** — name, submit_handler_ref. Digit type: `form.container`.
- **InputType** — Text, Number, Email, Date, Password, Multiline.
- All form schemas require `label` (accessibility enforcement).

### Rich Text System (`richtext.rs`)
Document block types for the Quill program.
- **HeadingMeta** — level (1-6), text. Digit type: `text.heading`.
- **ParagraphMeta** — text. Digit type: `text.paragraph`.
- **ListMeta** — style, items. Digit type: `text.list`.
- **BlockquoteMeta** — text, attribution. Digit type: `text.blockquote`.
- **CalloutMeta** — text, style. Digit type: `text.callout`.
- **CodeBlockMeta** — code, language. Digit type: `text.code`.
- **FootnoteMeta** — marker, text. Digit type: `text.footnote`.
- **CitationMeta** — source, url, author. Digit type: `text.citation`.
- **ListStyle** — Ordered, Unordered, Checklist.

### Interactive System (`interactive.rs`)
Interactive UI elements used across programs.
- **ButtonMeta** — label, action_ref, style. Digit type: `interactive.button`.
- **NavLinkMeta** — label, target_ref. Digit type: `interactive.nav-link`.
- **AccordionMeta** — title, expanded. Digit type: `interactive.accordion`.
- **TabGroupMeta** — tabs, active_index. Digit type: `interactive.tab-group`.
- **ButtonStyle** — Primary, Secondary, Tertiary, Danger, Custom(String).
- All interactive schemas require `label` or `title`.

### Commerce System (`commerce.rs`)
Marketplace types for Cart (Scry) and storefronts (Throne).
- **ProductMeta** — title, description, price_cents, seller_pubkey, images, categories, inventory. Digit type: `commerce.product`.
- **StorefrontMeta** — owner_pubkey, name, description, theme_ref. Digit type: `commerce.storefront`.
- **CartItemMeta** — product_ref, quantity, seller_pubkey, price_snapshot. Digit type: `commerce.cart-item`. Cart is local, consent-gated (sellers never see it until checkout).
- **OrderMeta** — buyer_pubkey, seller_pubkey, items, total_cents, status, payment_ref. Digit type: `commerce.order`.
- **ReviewMeta** — rating (1-5), text, author_pubkey, product_ref. Digit type: `commerce.review`.
- **OrderStatus** — Placed, Paid, Preparing, Shipped, Delivered, Confirmed, Disputed.

### Data Binding System (`binding.rs`)
Cross-file DataSource bindings connecting digits to data in other .idea files.
- **DataBinding** — source_ref, source_path, transform, live. Stored with `binding_` prefix in Digit properties.
- `with_data_binding(digit, binding, author) -> Digit` — attaches binding.
- `parse_data_binding(digit) -> Option<DataBinding>` — extracts binding.
- `BondRelationship::DataSource` — new bond variant for data source references.

### Shared Helpers (`helpers.rs`)
Property extraction helpers used by all domain modules.
- `prop_str`, `prop_str_opt` — string extraction.
- `prop_int`, `prop_int_opt` — integer extraction.
- `prop_double`, `prop_double_opt` — double extraction.
- `prop_bool`, `prop_bool_opt` — boolean extraction.
- `prop_str_array` — string array extraction.
- `check_type` — digit type validation.
- `make_error` — domain-specific error factory.

## Domain Digit Pattern
All domain modules follow the same pattern established by `media.rs`:
1. **Meta struct** — plain Rust struct with domain fields.
2. **Constructor** — `fn type_digit(meta, author) -> Result<Digit>` creates a digit with properties set.
3. **Parser** — `fn parse_type_meta(digit) -> Result<Meta>` extracts metadata from properties.
4. **Schema** — `fn type_schema() -> DigitSchema` defines the validation blueprint.
5. **Tests** — round-trip, wrong-type rejection, missing-property rejection, serde round-trip, schema validation.

## Still to Come

- ~~Encryption of content files~~ — **DONE** (Hall crate: Scribe/Scholar encrypt/decrypt via Sentinal)
- ~~Babel obfuscation~~ — **DONE** (Hall crate: Archivist uses Sentinal's obfuscation for assets)
- ~~Asset storage (shuffled binaries)~~ — **DONE** (Hall crate: Archivist with SHA-256 + Babel + AES-GCM -> .shuffled)
- ~~Schema system~~ — **DONE** (DigitSchema, PropertyType, SchemaRegistry, validation, versioning, composability)
- ~~Media digit helpers~~ — **DONE** (ImageMeta, AudioMeta, VideoMeta, StreamMeta with constructors + parsers)
- ~~Domain digit types~~ — **DONE** (sheet, slide, form, richtext, interactive, commerce)
- ~~Accessibility metadata~~ — **DONE** (AccessibilityMetadata, a11y_ properties, Digit::accessibility())
- ~~Data bindings~~ — **DONE** (DataBinding, binding_ properties, BondRelationship::DataSource)
- ~~Property key regex fix~~ — **DONE** (hyphens now allowed: `[a-zA-Z][a-zA-Z0-9_-]*`)
- IdeaSocket lifecycle orchestration (depends on Equipment/Pact)
- Layout system

## Covenant Alignment

**Sovereignty** — ownership is cryptographic, not platform-granted. **Dignity** — every creation has provenance and economic value from birth. **Consent** — key slots control who can decrypt; the creator decides. **Accessibility** — a Covenant duty; every interactive element requires a label.

## Dependencies

```toml
x = { path = "../X" }     # Value, VectorClock, CrdtOperation
uuid, chrono, serde, serde_json, base64, regex, thiserror, log
```

X is the only internal Omninet dependency. Future: Equipment (for IdeaSocket), Sentinal (for actual encryption).
