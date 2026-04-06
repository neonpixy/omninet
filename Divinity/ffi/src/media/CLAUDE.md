# Divinity FFI Media — Rust C FFI for Audio & Video (Plan)

No source files exist yet. This directory contains only this plan doc.

extern "C" functions exposing Communicator + audio codec operations to all platforms. Built when Throne apps need media capabilities.

Follows existing FFI patterns: `divi_` prefix, opaque pointers, JSON for complex types, raw bytes for audio frames, i32 return codes, `divi_last_error()` for details.

## Planned FFI Surface

### Communicator FFI
```
divi_communicator_new() -> *mut Communicator
divi_communicator_free(ptr)
divi_communicator_offer(ptr, channel_id, participants_json) -> i32
divi_communicator_accept(ptr, session_id) -> i32
divi_communicator_end(ptr, session_id) -> i32
divi_communicator_active_sessions(ptr) -> *mut c_char  // JSON
divi_communicator_session(ptr, session_id) -> *mut c_char  // JSON or null
```

### Opus FFI
```
divi_opus_encoder_new(config_json) -> *mut OpusEncoder
divi_opus_encoder_free(ptr)
divi_opus_encoder_encode(ptr, pcm, pcm_len, out, out_len) -> i32
divi_opus_decoder_new(sample_rate, channels) -> *mut OpusDecoder
divi_opus_decoder_free(ptr)
divi_opus_decoder_decode(ptr, data, data_len, out, out_len) -> i32
```

### Frame Encryption FFI
```
divi_encrypt_frame(data, len, key, out, out_len) -> i32
divi_decrypt_frame(data, len, key, out, out_len) -> i32
divi_free_bytes(ptr)  // already exists in helpers.rs
```

### Chunked Assets FFI

Rust infrastructure exists: `globe::ChunkManifest`, `ChunkInfo`, `ChunkBuilder` (split/verify/missing_chunks), kind 9000.

```
divi_chunk_manifest_split(data, data_len, chunk_size) -> *mut c_char  // JSON ChunkManifest
divi_chunk_manifest_parse(json) -> *mut c_char  // validated JSON
divi_chunk_manifest_verify(data, data_len, manifest_json) -> i32
divi_chunk_manifest_missing(manifest_json, available_json) -> *mut c_char  // JSON ChunkInfo[]
```

### SFU FFI

Rust infrastructure exists: `globe::SfuRouter`, `SfuSession`, `SfuParticipant`, `MediaLayer`, `LayerPreference`, `ForwardTarget`.

```
divi_sfu_router_new() -> *mut SfuRouter
divi_sfu_router_free(ptr)
divi_sfu_create_session(ptr, session_id, config_json) -> i32
divi_sfu_add_participant(ptr, session_id, participant_json) -> i32
divi_sfu_remove_participant(ptr, session_id, crown_id) -> i32
divi_sfu_publish_layers(ptr, session_id, crown_id, layers_json) -> i32
divi_sfu_set_preference(ptr, session_id, receiver, sender, pref_json) -> i32
divi_sfu_route(ptr, session_id, sender, layer_id) -> *mut c_char  // JSON ForwardTarget[]
divi_sfu_end_session(ptr, session_id) -> i32
```

## Dependencies
- Equipment (Communicator)
- Globe (ChunkManifest, ChunkBuilder, SfuRouter, SignalingBuilder)
- Sentinal (frame encryption via AES-256-GCM)
