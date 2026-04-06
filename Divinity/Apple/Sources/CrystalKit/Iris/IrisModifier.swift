// IrisModifier.swift
// CrystalKit
//
// SwiftUI ViewModifier and View extensions for the Iris material.
// Applies a thin-film interference effect to any view's background.
// Includes Gleam tracking for cursor-responsive dimple shifting.

import SwiftUI

struct IrisModifier: ViewModifier {
    var style: IrisStyle
    let shape: ShapeDescriptor

    #if os(macOS)
    @State private var gleamUnit: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @State private var viewSize: CGSize = .zero
    #endif

    func body(content: Content) -> some View {
        content
            .background {
                GeometryReader { geo in
                    MaterialRepresentable(
                        shape: shape,
                        isAnimating: style.animated,
                        configureView: { view in
                            let renderer = IrisRenderer(device: view.device!)
                            renderer.style = style
                            view.renderer = renderer
                            view.onBeforeRender = { elapsed in
                                renderer.time = elapsed
                            }
                        },
                        updateView: { view in
                            if let renderer = view.renderer as? IrisRenderer {
                                if renderer.style != style {
                                    renderer.style = style
                                    view.setNeedsRender()
                                }
                                #if os(macOS)
                                let unitPos = SIMD2<Float>(
                                    Float(gleamUnit.x),
                                    Float(gleamUnit.y)
                                )
                                if renderer.gleamPosition != unitPos {
                                    renderer.gleamPosition = unitPos
                                    view.setNeedsRender()
                                }
                                #endif
                            }
                        }
                    )
                    .allowsHitTesting(false)
                    #if os(macOS)
                    .onAppear { viewSize = geo.size }
                    .onChange(of: geo.size) { _, newSize in viewSize = newSize }
                    #endif
                }
                #if os(macOS)
                .onContinuousHover { phase in
                    switch phase {
                    case .active(let location):
                        guard viewSize.width > 0, viewSize.height > 0 else { return }
                        gleamUnit = CGPoint(
                            x: max(0, min(1, location.x / viewSize.width)),
                            y: max(0, min(1, location.y / viewSize.height))
                        )
                    case .ended:
                        gleamUnit = CGPoint(x: 0.5, y: 0.5)
                    @unknown default:
                        break
                    }
                }
                #endif
            }
    }
}

// MARK: - View Extensions

extension View {

    /// Applies a thin-film interference effect with the default style.
    public func iris() -> some View {
        modifier(IrisModifier(style: IrisStyle(), shape: .roundedRect()))
    }

    /// Applies a thin-film interference effect with the given style.
    public func iris(_ style: IrisStyle) -> some View {
        modifier(IrisModifier(style: style, shape: .roundedRect()))
    }

    /// Applies a thin-film interference effect with the given style and shape.
    public func iris(_ style: IrisStyle = IrisStyle(), shape: ShapeDescriptor) -> some View {
        modifier(IrisModifier(style: style, shape: shape))
    }

    /// Applies iris with a SwiftUI RoundedRectangle.
    public func iris(_ style: IrisStyle = IrisStyle(), in shape: RoundedRectangle) -> some View {
        let radius = shape.cornerSize.width
        return modifier(IrisModifier(style: style, shape: .roundedRect(cornerRadius: radius)))
    }

    /// Applies iris with a Circle.
    public func iris(_ style: IrisStyle = IrisStyle(), in shape: Circle) -> some View {
        modifier(IrisModifier(style: style, shape: .circle))
    }

    /// Applies iris with a Capsule.
    public func iris(_ style: IrisStyle = IrisStyle(), in shape: Capsule) -> some View {
        modifier(IrisModifier(style: style, shape: .capsule))
    }

    /// Applies iris with an Ellipse.
    public func iris(_ style: IrisStyle = IrisStyle(), in shape: Ellipse) -> some View {
        modifier(IrisModifier(style: style, shape: .ellipse))
    }
}
