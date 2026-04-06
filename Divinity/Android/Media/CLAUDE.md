# Divinity/Android Media — Kotlin Audio & Video Wrappers (Plan)

No source files exist yet. This directory contains only this plan doc.

Kotlin wrappers with JNI bridge for Rust media FFI. Built when Throne apps need media capabilities.

Follows existing Android/ patterns: AutoCloseable, ptr as Long, JNI bridge in C, functional interfaces for callbacks.

## Audio
- Oboe (C++ NDK, preferred) or AudioRecord for capture/playback
- Communicator.kt — AutoCloseable wrapper for divi_communicator_* JNI functions

## Video
- ExoPlayer with custom DataSource for chunked playback
- MediaCodec API for hardware transcoding (H.264/HEVC)
- BitmapFactory / MediaMetadataRetriever for thumbnails

## Live Video
- Google WebRTC Android SDK for calls
- CameraX API -> RTCVideoTrack

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — large file chunking (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
