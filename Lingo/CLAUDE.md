# Lingo — Language & Translation

The shared tongue. Lingo handles semantic text obfuscation (Babel), omnilingual tokenization, platform translation orchestration, language detection, and spreadsheet formula parsing/evaluation.

## Source Layout

```
Lingo/
├── Cargo.toml
├── src/
│   ├── lib.rs          ← module declarations + re-exports
│   ├── error.rs        ← LingoError enum
│   ├── symbols.rs      ← Full-power Unicode symbol space (90K+ symbols, LazyLock cached)
│   ├── tokenizer.rs    ← Omnilingual tokenizer (Script enum, space/char/grapheme splitting)
│   ├── vocabulary.rs   ← Hardened vocabulary: homophones + polyalphabetic byte alphabets
│   ├── babel.rs        ← Hardened encode/decode: nonce per encode, stream cipher security
│   ├── types.rs        ← TranslatedText, LanguageInfo, TranslationKit, CacheStatistics
│   ├── detection.rs    ← Language detection heuristics (9 languages)
│   ├── provider.rs     ← TranslationProvider trait (platform bridge)
│   ├── cache.rs        ← LRU translation cache
│   ├── translator.rs   ← UniversalTranslator orchestrator
│   └── formula/        ← Spreadsheet formula engine (Phase 1F)
│       ├── mod.rs      ← module declarations + re-exports
│       ├── error.rs    ← FormulaError enum with position spans
│       ├── value.rs    ← FormulaValue enum (Number, Text, Bool, Date, Error, Empty)
│       ├── ast.rs      ← FormulaNode AST, FormulaCellRef, BinaryOp, UnaryOp
│       ├── token.rs    ← FormulaToken enum, FormulaTokenizer
│       ├── parser.rs   ← FormulaParser (recursive descent + Pratt precedence)
│       ├── evaluator.rs ← FormulaEvaluator + CellResolver trait
│       ├── functions.rs ← FunctionRegistry + 23 built-in functions
│       ├── dependency.rs ← DependencyGraph, circular reference detection
│       └── locale.rs   ← FormulaLocale (function name translation, separators)
└── tests/
    └── integration.rs
```

## Architecture

```
UniversalTranslator (orchestrator)
    ├── Babel (hardened semantic text obfuscation)
    │   ├── Vocabulary (seed → token↔symbol mapping, hardened)
    │   │   ├── Homophones (up to 4 symbols per common token)
    │   │   ├── Polyalphabetic byte encoding (8 independent 256-symbol alphabets)
    │   │   ├── Symbols (90K+ Unicode from 60+ ancient/exotic blocks)
    │   │   └── SeededRandom (from Sentinal, Fisher-Yates shuffle)
    │   ├── Nonce (16-byte random per encode, embedded in output)
    │   ├── getrandom (OS randomness for nonce + homophone selection)
    │   └── Tokenizer (omnilingual: Latin/Arabic/Cyrillic→whitespace, CJK/Kana/Hangul→char, Thai→grapheme)
    ├── TranslationProvider trait (platform translation via Divinity FFI)
    └── TranslationCache (LRU, keyed by content_id + target_language)
```

## Babel Hardening

Three layers that together approach stream cipher security:

1. **Homophones** — Each common token maps to up to 4 symbols. Encoder picks randomly from the pool via `getrandom`. "the" could be any of 4 different symbols. Decoder handles any homophone transparently (reverse map has all variants). Defeats word frequency analysis.

2. **Polyalphabetic byte encoding** — 8 independent byte alphabets instead of 1. For unknown tokens (byte-encoded), each byte position uses a different alphabet selected by a nonce-derived `SeededRandom`. Same byte at different positions produces different symbols. Defeats letter frequency analysis.

3. **Nonce per encode** — Every `encode()` generates a random 16-byte nonce via `getrandom`. The nonce seeds the polyalphabetic alphabet selection and is embedded as the first token in the output. Same input produces different output every time. Defeats known-plaintext and replay attacks.

**Output format:** `[nonce_token] [content_token] [content_token] ...`
- Nonce is always the first space-separated token (16 bytes byte-encoded with alphabet 0)
- Decoder extracts nonce, reconstructs the alphabet sequence, decodes content

**Deterministic fallback:** `Vocabulary::encode/decode` (public API) uses first homophone + alphabet 0 for nuclear-proof determinism. `Babel::encode_token/decode_symbol` also use this path. Only the full-text `Babel::encode/decode` is non-deterministic.

## Key Design Decisions

1. **No canonical English.** Content is stored in its original language with a BCP 47 tag. Translation happens on read, from source language to reader's language. Every language is first-class. No round-trip translation loss.

2. **Dictionary is math, not data.** The vocabulary mapping is `f(seed, token) -> symbols`. Pure function. Drop and regenerate identically from the same seed. Nuclear-proof — nothing to corrupt, nothing to lose.

3. **Omnilingual tokenization.** Space-based for Latin/Arabic/Cyrillic/Devanagari. Character-level for CJK/Kana/Hangul. Grapheme clusters for Thai. Mixed text is handled by whitespace split first, then sub-tokenize CJK/Thai segments.

4. **Full-power Unicode.** 90,000+ symbols from 60+ Unicode blocks. Symbol budget: 8x256=2,048 for byte alphabets + ~19Kx4=~76K for homophones. Fits within 80K+ available symbols.

5. **Binary XOR / image ops stay in Sentinal.** Lingo only handles text vocabulary mapping. Sentinal owns byte-level and pixel-level obfuscation.

6. **TranslationProvider trait for platform bridges.** Apple Translation, Android ML Kit, etc. plug in via Divinity FFI. If no provider, text renders in its original language.

## Key Types

- **Babel** — Hardened text encode/decode. `new(seed)`, `encode(text)` (non-deterministic), `decode(encoded)`. Stores seed for nonce derivation.
- **Vocabulary** — Seed-deterministic vocabulary with homophones + polyalphabetic byte alphabets. Public `encode/decode` are deterministic (first homophone, alphabet 0). Crate-internal API exposes building blocks for Babel.
- **UniversalTranslator** — Orchestrates Babel + TranslationProvider + Cache. Builder pattern.
- **TranslationProvider** — Trait for platform translation. Object-safe for `Box<dyn>`.
- **TranslationCache** — LRU cache keyed by `(Uuid, String)`.
- **TranslatedText** — Result of translation (text, original, source/target language, from_cache).
- **TranslationKit** — Shared vocabulary for Babel decoding between parties (from crown_id, to crown_id, seed, signature).
- **StoredText** — Result of prepare_for_storage (text, source_language, babel_encoded).

## Formula Engine

Spreadsheet formula parsing, evaluation, and dependency tracking for Abacus and any other context that needs formulas.

### Architecture

```
Input ("=SUM(A1:A10)")
    │
    ▼
FormulaTokenizer  (token.rs)   → Vec<(FormulaToken, Span)>
    │
    ▼
FormulaParser     (parser.rs)  → FormulaNode (AST)
    │
    ▼
FormulaEvaluator  (evaluator.rs) + CellResolver → FormulaValue
```

### Key Design Decisions

1. **Independent of Ideas.** Uses its own `FormulaCellRef` (not Ideas' CellAddress) and `FormulaValue` (not x::Value). Keeps Lingo dependency-free from Ideas.
2. **Hand-rolled recursive descent parser** with Pratt-style operator precedence. No parser combinator libraries.
3. **CellResolver trait** lets the spreadsheet layer plug in cell data without Lingo knowing about the data model.
4. **Locale-aware.** Function names translate (SUM->SOMME in French, SUM->SUMME in German). Formulas stored in canonical English, displayed in the user's language.
5. **FormulaValue has Error variant.** Spreadsheet errors (#REF!, #DIV/0!, #VALUE!, #NAME?, #N/A, #NUM!, #CIRCULAR!) propagate through evaluation.

### Operator Precedence (lowest to highest)

1. Comparison (=, <>, <, <=, >, >=)
2. Concatenation (&)
3. Addition/Subtraction (+, -)
4. Multiplication/Division (*, /)
5. Exponentiation (^) — right-associative
6. Unary (-, %)
7. Primary (literals, cell refs, function calls, parenthesized)

### Built-in Functions (23)

- **Math:** SUM, AVERAGE, MIN, MAX, COUNT, ROUND, ABS, MOD, POWER
- **Text:** CONCAT, LEFT, RIGHT, MID, LEN, UPPER, LOWER, TRIM
- **Logic:** IF, AND, OR, NOT
- **Aggregate:** COUNTIF, SUMIF

### Key Types

- **FormulaParser** — `parse(input) -> Result<FormulaNode, FormulaError>`. Entry point.
- **FormulaEvaluator** — `evaluate(node, resolver) -> FormulaValue`. Walks the AST.
- **CellResolver** — Trait. `resolve(cell_ref) -> FormulaValue`, `resolve_range(start, end) -> Vec<FormulaValue>`.
- **FormulaCellRef** — Lingo's own cell reference type (column, row, abs flags, optional sheet).
- **FormulaValue** — Number, Text, Bool, Date, Error, Empty.
- **DependencyGraph** — Tracks cell-to-cell dependencies. `has_circular()`, `evaluation_order()`.
- **FormulaLocale** — Function name translation + separator conventions.
- **FunctionRegistry** — Pluggable function registry. `register(name, fn, arg_count)`.

## What Does NOT Live Here

- **Binary XOR obfuscation** -> Sentinal (already implemented)
- **Image color scrambling / pixel shuffling** -> Sentinal (already implemented)
- **BabelConfig in .idea headers** -> Ideas (serialization types)
- **Vocabulary seed derivation** -> Sentinal (`derive_vocabulary_seed`)
- **Secure Enclave / biometric** -> Divinity/Apple
- **NIP-44 encrypted DMs** -> Globe

## Dependencies

```toml
sentinal = { path = "../Sentinal" }  # SeededRandom, derive_vocabulary_seed
getrandom = "0.3"                    # OS randomness for nonce + homophone selection
sha2, unicode-segmentation, serde, serde_json, thiserror, uuid, chrono, log, bip39
```

Sentinal is the only internal Omninet dependency. Lingo depends on it for `SeededRandom` and vocabulary seed derivation, plus `getrandom` for OS randomness.

## Covenant Alignment

**Dignity** — everyone participates in their own language. No language is privileged. **Sovereignty** — translation happens on-device via platform providers. Vocabulary is derived from your key. **Consent** — Babel vocabularies are shared only via explicit TranslationKit exchange.
