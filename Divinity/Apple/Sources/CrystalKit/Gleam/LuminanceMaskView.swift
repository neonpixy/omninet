// GleamMaskView.swift
// CrystalKit
//
// Displays a Metal luminance mask texture as a SwiftUI view.
// Used by FacetModifier to composite per-pixel adaptive foreground:
// the mask's R channel drives opacity of the black foreground overlay.

import SwiftUI
import Metal

#if os(macOS)
import AppKit

/// Renders a `MTLTexture` as an `NSView` layer. The texture is expected to be
/// a BGRA luminance mask where brighter = show black foreground overlay.
/// The `generation` counter forces SwiftUI to call `updateNSView` even when
/// the texture object pointer hasn't changed (contents are updated in-place).
struct GleamMaskView: NSViewRepresentable {
    let texture: MTLTexture?
    let generation: UInt64
    /// Optional UV crop region (0-1) within the texture. When nil, the full texture is displayed.
    /// Used by goo participants to display only their sub-region of a full-scope mask.
    var cropRegion: CGRect? = nil

    func makeNSView(context: Context) -> GleamMaskNSView {
        GleamMaskNSView()
    }

    func updateNSView(_ nsView: GleamMaskNSView, context: Context) {
        nsView.updateTexture(texture, cropRegion: cropRegion)
    }
}

final class GleamMaskNSView: NSView {
    private var cachedImage: CGImage?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.contentsGravity = .resizeAspectFill
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        layer?.contentsScale = window?.backingScaleFactor ?? 2.0
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        layer?.contentsScale = window?.backingScaleFactor ?? 2.0
    }

    func updateTexture(_ texture: MTLTexture?, cropRegion: CGRect? = nil) {
        guard let texture else {
            cachedImage = nil
            layer?.contents = nil
            return
        }

        // Always re-blit: the renderer reuses the same texture object
        // but writes new luminance data each frame.
        cachedImage = cgImage(from: texture)
        layer?.contents = cachedImage

        if let crop = cropRegion {
            // contentsRect is in unit coordinate space (0-1).
            // macOS layers are not flipped by default, so Y=0 is bottom.
            // The texture has Y=0 at top, so flip the Y origin.
            layer?.contentsRect = CGRect(
                x: crop.origin.x,
                y: 1.0 - crop.origin.y - crop.height,
                width: crop.width,
                height: crop.height
            )
        } else {
            layer?.contentsRect = CGRect(x: 0, y: 0, width: 1, height: 1)
        }
    }

    /// Blits a managed Metal texture to a CGImage for layer display.
    private func cgImage(from texture: MTLTexture) -> CGImage? {
        let w = texture.width
        let h = texture.height
        guard w > 0, h > 0 else { return nil }

        let bytesPerRow = w * 4
        let byteCount = bytesPerRow * h
        let data = UnsafeMutableRawPointer.allocate(byteCount: byteCount, alignment: 16)

        texture.getBytes(data, bytesPerRow: bytesPerRow,
                         from: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0)

        // CGDataProvider takes ownership of the buffer — release in its callback.
        guard let provider = CGDataProvider(dataInfo: nil,
                                            data: data,
                                            size: byteCount,
                                            releaseData: { _, ptr, _ in ptr.deallocate() }) else {
            data.deallocate()
            return nil
        }

        // BGRA → we need to tell CGImage the component order
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)

        return CGImage(width: w, height: h,
                       bitsPerComponent: 8, bitsPerPixel: 32,
                       bytesPerRow: bytesPerRow,
                       space: CGColorSpaceCreateDeviceRGB(),
                       bitmapInfo: bitmapInfo,
                       provider: provider,
                       decode: nil, shouldInterpolate: true,
                       intent: .defaultIntent)
    }
}

#elseif os(iOS) || os(visionOS)
import UIKit

struct GleamMaskView: UIViewRepresentable {
    let texture: MTLTexture?
    let generation: UInt64
    /// Optional UV crop region (0-1) within the texture. When nil, the full texture is displayed.
    /// Used by goo participants to display only their sub-region of a full-scope mask.
    var cropRegion: CGRect? = nil

    func makeUIView(context: Context) -> LuminanceMaskUIView {
        LuminanceMaskUIView()
    }

    func updateUIView(_ uiView: LuminanceMaskUIView, context: Context) {
        uiView.updateTexture(texture, cropRegion: cropRegion)
    }
}

final class LuminanceMaskUIView: UIView {
    private var cachedImage: CGImage?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        isUserInteractionEnabled = false
        layer.contentsGravity = .resizeAspectFill
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
    }

    func updateTexture(_ texture: MTLTexture?, cropRegion: CGRect? = nil) {
        guard let texture else {
            cachedImage = nil
            layer.contents = nil
            return
        }

        cachedImage = cgImage(from: texture)
        layer.contents = cachedImage

        if let crop = cropRegion {
            // contentsRect is in unit coordinate space (0-1).
            // UIKit layers have Y=0 at top, matching the texture coordinate space.
            layer.contentsRect = crop
        } else {
            layer.contentsRect = CGRect(x: 0, y: 0, width: 1, height: 1)
        }
    }

    private func cgImage(from texture: MTLTexture) -> CGImage? {
        let w = texture.width
        let h = texture.height
        guard w > 0, h > 0 else { return nil }

        let bytesPerRow = w * 4
        let byteCount = bytesPerRow * h
        let data = UnsafeMutableRawPointer.allocate(byteCount: byteCount, alignment: 16)

        texture.getBytes(data, bytesPerRow: bytesPerRow,
                         from: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0)

        guard let provider = CGDataProvider(dataInfo: nil,
                                            data: data,
                                            size: byteCount,
                                            releaseData: { _, ptr, _ in ptr.deallocate() }) else {
            data.deallocate()
            return nil
        }

        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)

        return CGImage(width: w, height: h,
                       bitsPerComponent: 8, bitsPerPixel: 32,
                       bytesPerRow: bytesPerRow,
                       space: CGColorSpaceCreateDeviceRGB(),
                       bitmapInfo: bitmapInfo,
                       provider: provider,
                       decode: nil, shouldInterpolate: true,
                       intent: .defaultIntent)
    }
}

#endif
