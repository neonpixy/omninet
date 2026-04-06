# Nexus — Federation & Interop

The bridge. Nexus connects Omninet to the existing internet — not to assimilate, but to coexist. Import your history, export your creations, communicate with people who haven't crossed over yet.

Publish (on Globe) is primary; export (via Nexus) is the legacy escape hatch. Deterministic templates, optional AI polish.

## Architecture

Three plug-and-play subsystems, each backed by a trait registry:

```
Nexus (federation & interop)
    ├── Export (Exporter trait + ExporterRegistry)
    │   ├── Text: Markdown, CSV, JSON, TXT, HTML, SVG
    │   ├── Office: DOCX, XLSX, PPTX, ODT, ODS, ODP
    │   ├── Media: PDF, PNG, JPG
    │   └── 15 exporters registered via with_defaults()
    ├── Import (Importer trait + ImporterRegistry)
    │   ├── Text: Markdown, CSV, JSON
    │   ├── Office: XLSX, DOCX, PPTX
    │   ├── Media: PDF (text extraction)
    │   └── 7 importers registered via with_defaults()
    └── Bridge (ProtocolBridge trait + BridgeRegistry)
        └── SMTP (RFC 5322 with MIME multipart)
```

## Key Types

### Traits (`traits.rs`)
- **Exporter** — `id()`, `display_name()`, `supported_formats()`, `export(&[Digit], Option<Uuid>, &ExportConfig) → Result<ExportOutput>`
- **Importer** — `id()`, `display_name()`, `supported_mime_types()`, `import(&[u8], &ImportConfig) → Result<ImportOutput>`
- **ProtocolBridge** — `id()`, `display_name()`, `bridge(&MailMessage, &BridgeConfig) → Result<BridgeResult>`

### Config (`config.rs`)
- **ExportFormat** — 15 variants: Pdf, Png, Jpg, Svg, Docx, Xlsx, Pptx, Odt, Ods, Odp, Csv, Json, Markdown, Txt, Html. Each has `extension()` and `mime_type()`.
- **ExportConfig** — format, quality (Draft/Standard/HighQuality), page_size, accessibility, optional Regalia theme.
- **ImportConfig** — author crown_id, merge strategy (Replace/Append).
- **BridgeConfig** — protocol, settings HashMap (e.g., from_email, address_map for SMTP).

### Output (`output.rs`)
- **ExportOutput** — `data: Vec<u8>`, `filename: String`, `mime_type: String`.
- **ImportOutput** — `digits: Vec<Digit>`, `root_id: Option<Uuid>`, `warnings: Vec<String>`. Builder: `with_warning()`.
- **BridgeResult** — `status: String`, `payload: Value`, `warnings: Vec<String>`.

### Registry (`registry.rs`)
- **ExporterRegistry** — HashMap<String, Box<dyn Exporter>>. `with_defaults()` registers all 15. `find_for_format()`, `export()`, `supported_formats()`.
- **ImporterRegistry** — HashMap<String, Box<dyn Importer>>. `with_defaults()` registers all 7. `find_for_mime()`, `import()`.
- **BridgeRegistry** — HashMap<String, Box<dyn ProtocolBridge>>. `with_defaults()` registers SMTP. `bridge()`.

### Profile (`profile.rs`)
- **ExportProfile** — Print, Office, Web, Source, Media, Data, Everything. Maps to format subsets via `profile_formats()`.

### Error (`error.rs`)
- **NexusError** — ExportFailed, ImportFailed, UnsupportedFormat, SerializationError, InvalidInput, IoError, ParseError, BridgeFailed.

## Export Plugins (`export/`)

| File | Exporter | ID | Formats | Notes |
|------|----------|----|---------|-------|
| `markdown.rs` | MarkdownExporter | `markdown` | Markdown | 13+ digit types, tree walk |
| `csv_export.rs` | CsvExporter | `csv` | Csv | Sheet/cell → RFC 4180, text fallback |
| `json.rs` | JsonExporter | `json` | Json | Direct serde_json serialization |
| `txt.rs` | TxtExporter | `txt` | Txt | extract_text(), newline-separated |
| `html.rs` | HtmlExporter | `html` | Html | Magic ProjectionContext + CSS embed |
| `pdf.rs` | PdfExporter | `nexus.pdf` | Pdf | printpdf 0.8 ops API, text only |
| `xlsx.rs` | XlsxExporter | `nexus.xlsx` | Xlsx | rust_xlsxwriter, sheet/cell mapping |
| `png.rs` | PngExporter | `nexus.png` | Png | Placeholder (Phase 7 GPU pipeline) |
| `jpg.rs` | JpgExporter | `nexus.jpg` | Jpg | Placeholder (Phase 7 GPU pipeline) |
| `svg.rs` | SvgExporter | `nexus.svg` | Svg | Hand-coded, 9 digit types |
| `docx.rs` | DocxExporter | `nexus.docx` | Docx | OOXML via quick_xml + zip |
| `pptx.rs` | PptxExporter | `nexus.pptx` | Pptx | OOXML slides via quick_xml + zip |
| `odp.rs` | OdpExporter | `nexus.odp` | Odp | ODF presentation via quick_xml + zip |
| `odt.rs` | OdtExporter | `nexus.odt` | Odt | ODF text document via quick_xml + zip |
| `ods.rs` | OdsExporter | `nexus.ods` | Ods | ODF spreadsheet via quick_xml + zip |

## Import Plugins (`import/`)

| File | Importer | ID | MIME Types | Notes |
|------|----------|----|------------|-------|
| `markdown.rs` | MarkdownImporter | `nexus.markdown.import` | text/markdown | pulldown-cmark, 9+ elements |
| `csv_import.rs` | CsvImporter | `nexus.csv.import` | text/csv | csv crate, first row = headers |
| `json_import.rs` | JsonImporter | `nexus.json.import` | application/json | Native round-trip |
| `xlsx.rs` | XlsxImporter | `nexus.xlsx.import` | application/vnd...sheet | calamine 0.26, multi-sheet |
| `docx.rs` | DocxImporter | `nexus.docx.import` | application/vnd...document | roxmltree, text + headings |
| `pptx.rs` | PptxImporter | `nexus.pptx.import` | application/vnd...presentation | roxmltree, slide ordering |
| `pdf.rs` | PdfImporter | `nexus.pdf.import` | application/pdf | lopdf, text extraction (lossy) |

## Bridge Plugins (`bridge/`)

| File | Bridge | ID | Protocol | Notes |
|------|--------|----|----------|-------|
| `smtp.rs` | SmtpBridge | `nexus.smtp.bridge` | SMTP | RFC 5322, MIME multipart, base64 |

## Dependencies

```toml
x, ideas, magic, equipment, regalia  # Internal
image, printpdf, lopdf, rust_xlsxwriter, calamine  # Media/office
csv, pulldown-cmark, roxmltree, quick-xml, zip  # Text/XML
serde, serde_json, thiserror, uuid  # Standard
```

## Design Decisions

- Exporters take `&[Digit]` + `Option<Uuid>`, not `&DocumentState`. Keeps Nexus decoupled from Magic — caller extracts digits before calling export.
- PNG/JPG are intentional placeholders — actual rasterization requires the Magic+Divinity GPU pipeline (Phase 7).
- PDF/DOCX/PPTX imports are text-only and lossy — appropriate since .idea is the primary format.
- ODF formats (ODT, ODS, ODP) use the same tooling: quick-xml for XML generation + zip for the ODF container.
- All registries follow Magic's `RendererRegistry` pattern: HashMap-backed, `with_defaults()` constructor.

## Future Bridges (Not Yet Built)

- **ActivityPub** — Mastodon, Pixelfed, Lemmy
- **AT Protocol** — Bluesky
- **RSS/Atom** — blogs, podcasts, news
- **Non-Omninet relays** — standard Nostr relays and clients

## Covenant Alignment

**Sovereignty** — your data, your formats. Export means you can always leave, taking everything. **Dignity** — quality exports respect the design intent (Regalia theming). **Consent** — bridge protocols are explicit opt-in registrations.
