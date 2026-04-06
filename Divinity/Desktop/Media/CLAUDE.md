# Divinity/Desktop Media — C++ Audio & Video Wrappers (Plan)

No source files exist yet. This directory contains only this plan doc.

C++ header-only wrappers for Rust media FFI. Built when Throne apps need media capabilities.

Follows existing Desktop/ patterns: namespace `divinity`, RAII via `std::unique_ptr`, callback trampolines via `std::function`.

## Audio
- PulseAudio/PipeWire (Linux), WASAPI (Windows) for capture/playback
- communicator.hpp — RAII wrapper for divi_communicator_* FFI

## Video
- GStreamer or FFmpeg for playback and transcoding (Linux)
- Media Foundation for playback and transcoding (Windows)

## Live Video
- libwebrtc C++ API (native, not wrapped) for calls

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — large file chunking (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
