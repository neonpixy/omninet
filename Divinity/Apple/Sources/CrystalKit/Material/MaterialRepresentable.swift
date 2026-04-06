// MaterialRepresentable.swift
// CrystalKit
//
// Generic SwiftUI bridge for any MaterialRenderer. Creates and updates
// a MaterialMetalView (macOS) or MaterialUIView (iOS/visionOS).

import SwiftUI

#if os(macOS)

struct MaterialRepresentable: NSViewRepresentable {
    var shape: ShapeDescriptor
    var isAnimating: Bool
    var configureView: (MaterialMetalView) -> Void
    var updateView: (MaterialMetalView) -> Void

    func makeNSView(context: Context) -> MaterialMetalView {
        let view = MaterialMetalView(frame: .zero)
        view.shape = shape
        view.isAnimating = isAnimating
        configureView(view)
        return view
    }

    func updateNSView(_ nsView: MaterialMetalView, context: Context) {
        if nsView.shape != shape { nsView.shape = shape }
        nsView.isAnimating = isAnimating
        updateView(nsView)
    }
}

#elseif canImport(UIKit)

struct MaterialRepresentable: UIViewRepresentable {
    var shape: ShapeDescriptor
    var isAnimating: Bool
    var configureView: (MaterialUIView) -> Void
    var updateView: (MaterialUIView) -> Void

    func makeUIView(context: Context) -> MaterialUIView {
        let view = MaterialUIView(frame: .zero)
        view.shape = shape
        view.isAnimating = isAnimating
        configureView(view)
        return view
    }

    func updateUIView(_ uiView: MaterialUIView, context: Context) {
        if uiView.shape != shape { uiView.shape = shape }
        uiView.isAnimating = isAnimating
        updateView(uiView)
    }
}

#endif
