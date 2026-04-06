# Hall -- Encrypted File I/O

The great library. Hall reads and writes `.idea` packages to and from disk with AES-256-GCM encryption. Headers are always browseable without a key (Sovereignty). Corrupted content doesn't destroy the whole idea (Dignity). Hall takes raw keys -- it does NOT manage key lifecycle (Vault does that).

## Architecture

```
Vault (derives keys)
    | passes raw &[u8] keys (32 bytes)
    v
Hall (reads/writes encrypted files)
    |-- Scribe: IdeaPackage -> encrypted .idea directory
    |-- Scholar: encrypted .idea directory -> ReadResult<IdeaPackage>
    +-- Archivist: binary assets -> SHA-256 -> Babel XOR -> AES-GCM -> .shuffled
```

### Source Layout

```
Hall/src/
  lib.rs          -- module declarations + re-exports (HallError, HallWarning, ReadResult)
  error.rs        -- HallError enum, ReadResult<T>, HallWarning
  scribe.rs       -- Write path (encrypted .idea directories)
  scholar.rs      -- Read path (graceful degradation)
  archivist.rs    -- Binary asset pipeline (SHA-256 + Babel + AES-GCM)
```

### Key Types

- **HallError** -- Error enum. Variants for corrupted header/digit/asset/authority/coinage/position, hash mismatch, encryption, directory creation, IO, serialization.
- **ReadResult\<T\>** -- Carries both the successfully-read data AND a list of warnings. `has_warnings()` check.
- **HallWarning** -- Non-fatal issue with message and optional file path.

### Encrypted .idea Package Format

```
MyDocument.idea/
  Header.json                 -- PLAINTEXT (always browseable without key)
  Content/
    {uuid}.json               -- ENCRYPTED (AES-256-GCM combined format)
  Authority/                  -- optional
    book.encrypted            -- ENCRYPTED
    tree.encrypted            -- ENCRYPTED
  Coinage/                    -- optional
    value.encrypted           -- ENCRYPTED
    redemption.encrypted      -- ENCRYPTED
  Bonds/                      -- optional
    local.json                -- PLAINTEXT (references, not content)
    private.json              -- PLAINTEXT
    public.json               -- PLAINTEXT
  Position/                   -- optional
    position.encrypted        -- ENCRYPTED
  Assets/                     -- optional
    {sha256hex}.shuffled      -- OBFUSCATED + ENCRYPTED
```

### Scribe (Write Path)

`hall::scribe::write(package: &IdeaPackage, content_key: &[u8]) -> Result<u64, HallError>`

1. Create .idea directory
2. Write `Header.json` as plaintext (always)
3. Each digit: JSON encode -> `encrypt_combined` -> `Content/{uuid}.json`
4. Authority: JSON encode -> encrypt -> `book.encrypted` / `tree.encrypted`
5. Coinage: JSON encode -> encrypt -> `value.encrypted` / `redemption.encrypted`
6. Bonds: plaintext JSON (references, not content)
7. Position: JSON encode -> encrypt -> `position.encrypted`
8. Returns total bytes written

Encrypted filename constants live in `scribe::encrypted_names`.

### Scholar (Read Path)

```rust
hall::scholar::read_header(path) -> Result<Header, HallError>     // no key needed
hall::scholar::read(path, key) -> Result<ReadResult<IdeaPackage>, HallError>
hall::scholar::is_idea_package(path) -> bool
```

**Graceful degradation:** Header read is fatal (required). Everything else is graceful -- corrupted digits/authority/coinage/position produce `HallWarning`s, not errors. `ReadResult<T>` carries both the successfully-read data AND a list of warnings. Reading with the wrong key returns a valid ReadResult with an empty digits map and warnings (header still loads since it's plaintext).

### Archivist (Asset Pipeline)

```
Import: data -> SHA-256(data) -> obfuscate(data, seed) -> encrypt(key) -> Assets/{hash}.shuffled
Read:   read .shuffled -> decrypt(key) -> deobfuscate(seed) -> SHA-256 verify -> data
```

Three layers: content addressing (SHA-256), semantic obfuscation (Babel XOR), encryption (AES-256-GCM). Hash verification on read is **mandatory** -- catches both corruption and tampering.

Public API: `import`, `import_file`, `read`, `export`, `list`, `exists`, `delete`.

## Dependencies

```toml
ideas = { path = "../Ideas" }        # IdeaPackage, Header, Digit, package::names constants
sentinal = { path = "../Sentinal" }  # encrypt/decrypt_combined, obfuscate/deobfuscate
sha2 = "0.10"                        # Asset content addressing
hex = "0.4"                          # SHA-256 hex encoding
serde, serde_json, thiserror, uuid, log
```

Dev-dependencies: tempfile, chrono, x (for test Value construction).

Hall has zero dependency on Vault, Equipment, or X at the library layer.

## What Does NOT Live Here

- **Key management / lifecycle** -- Vault (derives keys, passes to Hall)
- **Password handling / PBKDF2** -- Sentinal + Vault
- **File watching** -- Deferred (platform-specific, needs `notify` crate)
- **Pact integration** -- Deferred (needs Throne apps)
- **CRDT operation log** -- Deferred (Ideas crate owns the format)
- **Streaming / chunked reads** -- Deferred (Babel requires full file for XOR)

## Covenant Alignment

**Sovereignty** -- Headers are always readable so you can browse your own library without decryption. **Dignity** -- corrupted content doesn't destroy the whole idea; graceful degradation preserves what's recoverable. **Consent** -- encryption keys control who can read content.
