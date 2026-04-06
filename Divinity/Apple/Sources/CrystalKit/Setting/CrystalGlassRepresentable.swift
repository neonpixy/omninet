// SettingRepresentable.swift
// CrystalKit
//
// Platform-specific NSViewRepresentable / UIViewRepresentable bridge
// that wraps the Metal-backed glass views for SwiftUI.

import SwiftUI
import simd

#if os(macOS)
import AppKit

struct SettingRepresentable: NSViewRepresentable {

    var style: FacetStyle
    var shape: ShapeDescriptor
    var gazePoint: CGPoint?
    @Binding var backgroundLuminance: CGFloat
    @Binding var backdropColor: SIMD3<Float>
    @Binding var luminanceMaskTexture: MTLTexture?
    @Binding var luminanceMaskGeneration: UInt64
    @Environment(\.bedrockProvider) var backdropProvider
    @Environment(\.facetOffset) var facetOffset

    func makeNSView(context: Context) -> SettingNSView {
        let view = SettingNSView(frame: .zero)
        view.style = style
        view.shape = shape
        view.backdropProvider = backdropProvider
        view.externalGazePoint = gazePoint
        view.externalOffset = facetOffset
        view.onLuminanceUpdate = { lum in
            DispatchQueue.main.async {
                self.backgroundLuminance = lum
            }
        }
        view.onBackdropColorUpdate = { color in
            DispatchQueue.main.async {
                self.backdropColor = color
            }
        }
        view.onLuminanceMaskUpdate = { tex in
            DispatchQueue.main.async {
                self.luminanceMaskTexture = tex
                self.luminanceMaskGeneration &+= 1
            }
        }
        return view
    }

    func updateNSView(_ nsView: SettingNSView, context: Context) {
        if nsView.style != style { nsView.style = style }
        if nsView.shape != shape { nsView.shape = shape }
        if nsView.backdropProvider !== backdropProvider { nsView.backdropProvider = backdropProvider }
        if nsView.externalGazePoint != gazePoint { nsView.externalGazePoint = gazePoint }
        if nsView.externalOffset != facetOffset { nsView.externalOffset = facetOffset }
    }
}

#elseif os(iOS) || os(visionOS)
import UIKit

struct SettingRepresentable: UIViewRepresentable {

    var style: FacetStyle
    var shape: ShapeDescriptor
    var gazePoint: CGPoint?
    @Binding var backgroundLuminance: CGFloat
    @Binding var backdropColor: SIMD3<Float>
    @Binding var luminanceMaskTexture: MTLTexture?
    @Binding var luminanceMaskGeneration: UInt64
    @Environment(\.bedrockProvider) var backdropProvider

    func makeUIView(context: Context) -> SettingUIView {
        let view = SettingUIView(frame: .zero)
        view.style = style
        view.shape = shape
        view.backdropProvider = backdropProvider
        view.externalGazePoint = gazePoint
        view.onLuminanceUpdate = { lum in
            DispatchQueue.main.async {
                self.backgroundLuminance = lum
            }
        }
        view.onBackdropColorUpdate = { color in
            DispatchQueue.main.async {
                self.backdropColor = color
            }
        }
        view.onLuminanceMaskUpdate = { tex in
            DispatchQueue.main.async {
                self.luminanceMaskTexture = tex
                self.luminanceMaskGeneration &+= 1
            }
        }
        return view
    }

    func updateUIView(_ uiView: SettingUIView, context: Context) {
        if uiView.style != style { uiView.style = style }
        if uiView.shape != shape { uiView.shape = shape }
        if uiView.backdropProvider !== backdropProvider { uiView.backdropProvider = backdropProvider }
        if uiView.externalGazePoint != gazePoint { uiView.externalGazePoint = gazePoint }
    }
}

#endif
