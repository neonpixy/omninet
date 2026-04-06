# Divinity/Microsoft Media — Platform Audio & Video (Plan)

No source files exist yet. This directory contains only this plan doc.

Native Windows media integration. Built when Throne apps need media capabilities.

## Audio
- WASAPI for capture/playback (exclusive mode for calls, shared mode for music)
- Rust bindings via `windows` crate

## Video
- Media Foundation for playback, transcoding (H.264/HEVC via MFT), and thumbnail extraction

## Live Video
- libwebrtc (native C++ library)
- Media Foundation for camera capture
- DXGI Desktop Duplication for screen capture

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — large file chunking (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
