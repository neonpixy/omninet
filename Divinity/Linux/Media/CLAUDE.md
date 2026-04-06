# Divinity/Linux Media — Platform Audio & Video (Plan)

No source files exist yet. This directory contains only this plan doc.

Native Linux media integration. Built when Throne apps need media capabilities.

## Audio
- PipeWire (preferred, low-latency) or PulseAudio (fallback) for capture/playback
- Rust bindings via `pipewire` or `libpulse-binding` crates

## Video
- GStreamer or FFmpeg for playback and transcoding
- Hardware-accelerated decoding via VA-API or VDPAU

## Live Video
- libwebrtc (native C++ library)
- V4L2 for camera capture
- PipeWire / XDG Desktop Portal for screen capture

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — large file chunking (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
