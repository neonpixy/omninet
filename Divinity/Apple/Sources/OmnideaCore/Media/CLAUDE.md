# Divinity/Apple Media — Platform Audio & Video (Plan)

No source files exist yet. This directory contains only this plan doc.

Thin Swift wrappers over Rust infrastructure. Built when Throne apps need media capabilities.

## Audio (prerequisite: Equipment Communicator + Opus + Globe signaling)

- **AudioCapture.swift** — AVAudioEngine mic capture -> PCM Int16. Voice processing for calls, disabled for music.
- **AudioPlayback.swift** — AVAudioEngine speaker output from PCM Int16. Configurable sample rate/channels.
- **Communicator.swift** — Swift wrapper for Communicator FFI. OpaquePointer pattern.

## Video (prerequisite: Globe chunked assets)

- **VideoTranscoder.swift** — AVAssetExportSession / VideoToolbox. H.264/HEVC presets.
- **ChunkedVideoPlayer.swift** — AVPlayer with custom AVAssetResourceLoaderDelegate. Decrypts chunks.
- **ThumbnailGenerator.swift** — CGImage resize for images, AVAssetImageGenerator for video posters.

## Live Video (prerequisite: Audio + Video + SFU)

- **WebRTCSession.swift** — Google WebRTC SDK. SDP offer/answer, ICE candidates, tracks.
- **CameraCapture.swift** — AVCaptureSession -> RTCVideoTrack. Front/back switching.
- **ScreenCapture.swift** — ReplayKit -> RTCVideoTrack. Screen sharing.
- **HLSSegmenter.swift** — AVAssetWriter -> 4-second HLS segments.
- **SegmentPlayer.swift** — Fetch segments from relay, feed to AVPlayer.

## Frameworks Required
- AVFoundation, VideoToolbox, ReplayKit, Google WebRTC SDK

## Rust Infrastructure Available
- `globe::ChunkManifest` / `ChunkBuilder` — split, verify, missing_chunks, manifest events (kind 9000)
- `globe::SfuRouter` — selective forwarding for group video (MediaLayer, LayerPreference)
- `globe::SignalingBuilder::ice_candidate()` — ICE candidate exchange (kind 5103)
