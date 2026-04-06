# Divinity/Web Media — WASM Audio & Video Wrappers (Plan)

No source files exist yet. This directory contains only this plan doc.

Rust WASM wrappers via wasm-bindgen + Web APIs. Built when Throne apps need media capabilities.

Follows existing Web/ patterns: #[wasm_bindgen], wraps crates directly (not through C FFI), auto-generates TypeScript.

## Audio
- Web Audio API (AudioWorklet) via web-sys for capture/playback
- getUserMedia for mic access
- communicator.rs — #[wasm_bindgen] wrapper for Equipment Communicator

## Video
- HTMLVideoElement with MediaSource Extensions (MSE) for chunked playback
- OffscreenCanvas for thumbnails

## Live Video
- Browser RTCPeerConnection via web-sys for calls
- getUserMedia for camera/mic

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — large file chunking (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
