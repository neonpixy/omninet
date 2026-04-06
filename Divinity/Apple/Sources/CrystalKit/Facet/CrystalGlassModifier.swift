// FacetModifier.swift
// CrystalKit
//
// SwiftUI ViewModifier that applies a Liquid Glass effect behind the content.
// Automatically adapts child foreground style (white/black) based on
// the background luminance behind the glass.
//
// Usage:
//   Text("Hello").facet()
//   Text("Hello").facet(.frosted)
//   Text("Hello").facet(.regular, in: Capsule())
//   Text("Hello").facet(FacetStyle(frost: 0.7, refraction: 0.4))
//   Text("Hello").facet(.init(lightSource: .cursor()))  // follows cursor/gaze

import SwiftUI
import Metal
import simd

// MARK: - Environment Keys

/// The average luminance (0-1) of the background behind the nearest Crystal Glass.
/// Dark backgrounds → low values, light backgrounds → high values.
/// Read this in child views to customize appearance beyond the automatic foreground style.
private struct FacetLuminanceKey: EnvironmentKey {
    static let defaultValue: CGFloat = 0.5
}

/// An externally-provided light source position in screen coordinates.
/// When set, overrides the built-in tracking (tilt on iPhone, hover on iPad, mouse on macOS).
/// Use only when you have a custom input source for the glass light direction.
private struct GleamPointKey: EnvironmentKey {
    static let defaultValue: CGPoint? = nil
}

// MARK: - Goo Environment Keys

private struct ConfluenceScopeActiveKey: EnvironmentKey {
    static let defaultValue: Bool = false
}

private struct ConfluenceEnabledKey: EnvironmentKey {
    static let defaultValue: Bool = false
}

private struct ConfluenceCrystallizedKey: EnvironmentKey {
    static let defaultValue: Bool = false
}

private struct FacetScopeActiveKey: EnvironmentKey {
    static let defaultValue: Bool = false
}

private struct ConfluenceOffsetKey: EnvironmentKey {
    static let defaultValue: CGSize = .zero
}

private struct FacetOffsetKey: EnvironmentKey {
    static let defaultValue: CGSize = .zero
}

private struct ConfluenceLuminanceMapKey: EnvironmentKey {
    static let defaultValue: [String: CGFloat] = [:]
}

/// Scope center in global (window) coordinates, published by ConfluenceScope.
/// Children use this to report positions in center-origin directly.
private struct ConfluenceScopeCenterKey: EnvironmentKey {
    static let defaultValue: CGPoint? = nil
}

/// The goo scope's last rendered output as a Metal texture.
/// On iOS, standalone glass views composite this into their captured backdrop
/// so that goo glass refracts correctly through standalone glass.
/// Nil on macOS (CGWindowListCreateImage captures Metal layers natively).
private struct ConfluenceOutputTextureKey: EnvironmentKey {
    nonisolated(unsafe) static let defaultValue: MTLTexture? = nil
}

/// The goo scope's frame in global (window) coordinates.
/// Used by standalone glass views to map the goo texture region correctly.
private struct ConfluenceScopeFrameKey: EnvironmentKey {
    static let defaultValue: CGRect = .zero
}

/// Per-child averaged backdrop color in goo context (sRGB 0-1).
private struct ConfluenceBackdropColorMapKey: EnvironmentKey {
    static let defaultValue: [String: SIMD3<Float>] = [:]
}

/// The goo scope's per-pixel luminance mask texture (for adaptive foreground).
private struct ConfluenceLuminanceMaskTextureKey: EnvironmentKey {
    nonisolated(unsafe) static let defaultValue: MTLTexture? = nil
}

/// Generation counter for the goo luminance mask (forces SwiftUI update).
private struct ConfluenceLuminanceMaskGenerationKey: EnvironmentKey {
    static let defaultValue: UInt64 = 0
}

/// The goo scope's size in points (for computing per-child UV crop regions).
private struct ConfluenceScopeSizeKey: EnvironmentKey {
    static let defaultValue: CGSize = .zero
}

/// The luminance mask texture from the nearest glass or goo scope.
/// Zone AF children sample this at their own center to pick white/black foreground.
private struct GleamMaskTextureKey: EnvironmentKey {
    nonisolated(unsafe) static let defaultValue: MTLTexture? = nil
}

/// The glass surface's frame in global coordinates.
/// Zone AF children use this to map their global center to UV within the mask.
private struct FacetFrameKey: EnvironmentKey {
    static let defaultValue: CGRect = .zero
}

/// Generation counter for the luminance mask.
/// The renderer reuses the same MTLTexture pointer, so pointer identity doesn't
/// change — this counter is the change signal that forces zone AF re-probing.
private struct GleamMaskGenerationKey: EnvironmentKey {
    static let defaultValue: UInt64 = 0
}

/// GPU-computed luminance probe results, keyed by child ID.
/// When populated, zone AF skips CPU getBytes and uses these directly.
private struct GleamProbeResultsKey: EnvironmentKey {
    static let defaultValue: [String: CGFloat] = [:]
}

/// The child ID of the nearest glass modifier, published so the AF modifier
/// can look up its GPU probe result.
private struct GleamChildIDKey: EnvironmentKey {
    static let defaultValue: String? = nil
}

extension EnvironmentValues {
    /// The average background luminance behind the nearest Crystal Glass effect (0-1).
    public var facetLuminance: CGFloat {
        get { self[FacetLuminanceKey.self] }
        set { self[FacetLuminanceKey.self] = newValue }
    }

    /// An external light source position in screen coordinates.
    /// When set, overrides built-in tracking (tilt on iPhone, hover on iPad, mouse on macOS).
    /// Only needed when providing a custom input source for the glass light direction.
    public var gleamPoint: CGPoint? {
        get { self[GleamPointKey.self] }
        set { self[GleamPointKey.self] = newValue }
    }

    var confluenceScopeActive: Bool {
        get { self[ConfluenceScopeActiveKey.self] }
        set { self[ConfluenceScopeActiveKey.self] = newValue }
    }

    var confluenceEnabled: Bool {
        get { self[ConfluenceEnabledKey.self] }
        set { self[ConfluenceEnabledKey.self] = newValue }
    }

    var confluenceCrystallized: Bool {
        get { self[ConfluenceCrystallizedKey.self] }
        set { self[ConfluenceCrystallizedKey.self] = newValue }
    }

    var facetScopeActive: Bool {
        get { self[FacetScopeActiveKey.self] }
        set { self[FacetScopeActiveKey.self] = newValue }
    }

    var confluenceLuminanceMap: [String: CGFloat] {
        get { self[ConfluenceLuminanceMapKey.self] }
        set { self[ConfluenceLuminanceMapKey.self] = newValue }
    }

    /// Scope center in global coordinates. Children subtract this from their
    /// own global center to get a center-origin position for the Metal renderer.
    var confluenceScopeCenter: CGPoint? {
        get { self[ConfluenceScopeCenterKey.self] }
        set { self[ConfluenceScopeCenterKey.self] = newValue }
    }

    /// An external offset applied to the goo child's reported frame.
    ///
    /// Use this when moving a goo participant with `.offset()` (which doesn't
    /// affect `GeometryReader.frame(in:)`). The glass modifier adds this to
    /// the frame it reports to the goo scope, keeping the Metal viewport in
    /// sync with the visual position.
    ///
    /// ```swift
    /// myView
    ///     .facet(.regular, in: Circle())
    ///     .confluence()
    ///     .confluenceOffset(dragOffset)
    ///     .offset(dragOffset)
    /// ```
    public var confluenceOffset: CGSize {
        get { self[ConfluenceOffsetKey.self] }
        set { self[ConfluenceOffsetKey.self] = newValue }
    }

    /// An external offset applied to standalone glass UV computation.
    ///
    /// SwiftUI's `.offset()` moves the view visually via a CALayer transform,
    /// but the underlying `SettingNSView` stays put — its `convert(bounds, to: nil)`
    /// doesn't see the transform. This key tells the Metal view about the offset
    /// so UV coordinates (and thus refraction) follow the visual position.
    ///
    /// Prefer the convenience `.facetMovable(offset:)` which applies both
    /// `.facetOffset()` and `.offset()` together.
    public var facetOffset: CGSize {
        get { self[FacetOffsetKey.self] }
        set { self[FacetOffsetKey.self] = newValue }
    }

    /// The goo scope's last rendered output as a Metal texture (iOS only).
    /// Standalone glass views use this to composite goo content into their
    /// captured backdrop, since `CALayer.render(in:)` can't capture Metal layers.
    var confluenceOutputTexture: MTLTexture? {
        get { self[ConfluenceOutputTextureKey.self] }
        set { self[ConfluenceOutputTextureKey.self] = newValue }
    }

    /// The goo scope's frame in global (window) coordinates.
    var confluenceScopeFrame: CGRect {
        get { self[ConfluenceScopeFrameKey.self] }
        set { self[ConfluenceScopeFrameKey.self] = newValue }
    }

    /// Per-child averaged backdrop color in goo context.
    var confluenceBackdropColorMap: [String: SIMD3<Float>] {
        get { self[ConfluenceBackdropColorMapKey.self] }
        set { self[ConfluenceBackdropColorMapKey.self] = newValue }
    }

    /// The goo scope's per-pixel luminance mask texture (for adaptive foreground).
    var confluenceLuminanceMaskTexture: MTLTexture? {
        get { self[ConfluenceLuminanceMaskTextureKey.self] }
        set { self[ConfluenceLuminanceMaskTextureKey.self] = newValue }
    }

    /// Generation counter for the goo luminance mask.
    var confluenceLuminanceMaskGeneration: UInt64 {
        get { self[ConfluenceLuminanceMaskGenerationKey.self] }
        set { self[ConfluenceLuminanceMaskGenerationKey.self] = newValue }
    }

    /// The goo scope's size in points.
    var confluenceScopeSize: CGSize {
        get { self[ConfluenceScopeSizeKey.self] }
        set { self[ConfluenceScopeSizeKey.self] = newValue }
    }

    /// The luminance mask texture from the nearest glass or goo scope.
    var gleamMaskTexture: MTLTexture? {
        get { self[GleamMaskTextureKey.self] }
        set { self[GleamMaskTextureKey.self] = newValue }
    }

    /// The glass surface's frame in global coordinates.
    var facetFrame: CGRect {
        get { self[FacetFrameKey.self] }
        set { self[FacetFrameKey.self] = newValue }
    }

    /// Generation counter for the luminance mask (forces zone AF re-probing).
    var gleamMaskGeneration: UInt64 {
        get { self[GleamMaskGenerationKey.self] }
        set { self[GleamMaskGenerationKey.self] = newValue }
    }

    /// GPU-computed luminance probe results, keyed by child ID.
    var gleamProbeResults: [String: CGFloat] {
        get { self[GleamProbeResultsKey.self] }
        set { self[GleamProbeResultsKey.self] = newValue }
    }

    /// The child ID of the nearest glass modifier.
    var gleamChildID: String? {
        get { self[GleamChildIDKey.self] }
        set { self[GleamChildIDKey.self] = newValue }
    }

}

// MARK: - Shared Goo Output (iOS)

/// Shared storage for the goo scope's rendered output texture and frame.
///
/// On iOS, `CALayer.render(in:)` can't capture `CAMetalLayer` drawables.
/// The goo view writes its offscreen output texture here; standalone glass
/// views read it during backdrop capture to composite goo content on CPU.
///
/// This uses `nonisolated(unsafe) static var` — the same pattern as
/// `GleamTiltTracker._sharedLightDirection`. Both the goo view and
/// standalone glass run on `@MainActor` (display link on main run loop),
/// so there's no data race. Using a static avoids the SwiftUI environment
/// limitation where siblings in a ZStack can't share state.
@MainActor
enum ConfluenceOutputStore {
    /// The goo scope's last rendered offscreen texture (`.shared` storage, BGRA).
    /// `nil` when no goo scope is active or on macOS (not needed).
    nonisolated(unsafe) static var texture: MTLTexture?

    /// The goo scope view's frame in window coordinates (points).
    /// Used to map goo texture pixels to the standalone glass view's region.
    nonisolated(unsafe) static var scopeFrame: CGRect = .zero

    /// Incremented by the goo view after each render pass.
    /// Standalone glass views compare their last-seen value to detect new goo content
    /// without relying on texture pointer identity (goo reuses the same allocation).
    nonisolated(unsafe) static var renderCount: UInt64 = 0
}

// MARK: - Live Child Store (frame bypass)

/// Shared storage for live child frames, written directly by glass modifiers
/// during `onChange(of: reportedFrame)` — which fires after SwiftUI layout is
/// complete. This bypasses the preference → @State pipeline (one frame late)
/// and triggers an immediate Metal re-render so the glass surface tracks
/// foreground content without visible lag during drag/scroll.
@MainActor
enum ConfluenceLiveChildStore {
    /// Latest child frames, written directly by glass modifiers during onChange.
    static var children: [String: ConfluenceChildInfo] = [:]

    #if os(macOS)
    /// The active goo NSView. Set by ConfluenceNSView on viewDidMoveToWindow.
    static weak var activeGooView: ConfluenceNSView?
    #endif

    /// When true, interaction effects are active and the TimelineView drives
    /// rendering. Luminance/color callbacks are suppressed to avoid a
    /// cascade: render → callback → @State → scope body → tick.
    static var interactionsActive = false

    // Per-child interaction data, written by the scope's TimelineView.
    // Children read these via per-child TimelineViews — no SwiftUI environment involved.
    static var interactionOffsets: [String: CGSize] = [:]
    static var interactionZIndices: [String: Double] = [:]
    static var interactionStyleMods: [String: FacetModification] = [:]

    static func update(_ child: ConfluenceChildInfo) {
        children[child.id] = child
        #if os(macOS)
        // Skip the immediate Metal re-render during interactions — the scope's
        // TimelineView already drives rendering via updateNSView with consistent
        // patchedConfluenceGroups every frame. Calling renderFromLiveChildren() here
        // would render with the OLD group structure + NEW live frames, causing
        // the glass to blip between positions.
        guard !interactionsActive else { return }
        activeGooView?.renderFromLiveChildren()
        #endif
    }

    static func remove(_ id: String) {
        children.removeValue(forKey: id)
    }
}

// MARK: - Goo Preference Key

struct ConfluenceChildInfo: Equatable {
    let id: String
    let frame: CGRect
    let style: FacetStyle
    let shape: ShapeDescriptor
    let crystallized: Bool
    var zIndex: Double = 0

    static func == (lhs: ConfluenceChildInfo, rhs: ConfluenceChildInfo) -> Bool {
        // Frame and zIndex intentionally excluded — they change every
        // interaction frame. Including them causes a preference → @State →
        // body → preference cascade. The live child store provides real-time
        // frames/zIndex for Metal rendering; the preference only needs identity.
        lhs.id == rhs.id
            && lhs.style == rhs.style
            && lhs.crystallized == rhs.crystallized
    }
}

struct ConfluenceChildrenKey: PreferenceKey {
    static let defaultValue: [ConfluenceChildInfo] = []
    static func reduce(value: inout [ConfluenceChildInfo], nextValue: () -> [ConfluenceChildInfo]) {
        value.append(contentsOf: nextValue())
    }
}

// MARK: - Modifier

struct FacetModifier: ViewModifier {

    var style: FacetStyle
    let shape: ShapeDescriptor

    @State private var backgroundLuminance: CGFloat = 0.5
    @State private var backdropColor: SIMD3<Float> = SIMD3(repeating: 0.5)
    @State private var childID = UUID().uuidString
    @State private var luminanceMaskTexture: MTLTexture?
    @State private var luminanceMaskGeneration: UInt64 = 0
    @State private var glassFrame: CGRect = .zero
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.gleamPoint) private var gazePoint
    @Environment(\.confluenceScopeActive) private var insideGooScope
    @Environment(\.confluenceEnabled) private var gooEnabled
    @Environment(\.confluenceLuminanceMap) private var gooLuminanceMap
    @Environment(\.confluenceBackdropColorMap) private var gooBackdropColorMap
    @Environment(\.confluenceOffset) private var gooOffset
    @Environment(\.confluenceScopeCenter) private var scopeCenter
    @Environment(\.confluenceCrystallized) private var crystalCrystallized
    @Environment(\.facetScopeActive) private var facetScopeActive
    @Environment(\.confluenceLuminanceMaskTexture) private var gooLuminanceMaskTexture
    @Environment(\.confluenceLuminanceMaskGeneration) private var gooLuminanceMaskGeneration
    @Environment(\.confluenceScopeSize) private var gooScopeSize
    @Environment(\.confluenceScopeFrame) private var confluenceScopeFrame
    // Interaction offset/zIndex/styleMod flow via ConfluenceLiveChildStore
    // (static) to avoid per-frame body cascade — children apply them in
    // per-child TimelineViews.

    private var isGooParticipant: Bool { insideGooScope && (gooEnabled || facetScopeActive) }

    /// Converts the Metal `ShapeDescriptor` back to a SwiftUI clip shape
    /// for the adaptive contrast layer.
    private var glassClipShape: AnyShape {
        switch shape {
        case .roundedRect(let radii, _):
            AnyShape(UnevenRoundedRectangle(cornerRadii: .init(
                topLeading: radii.topLeft,
                bottomLeading: radii.bottomLeft,
                bottomTrailing: radii.bottomRight,
                topTrailing: radii.topRight
            )))
        case .capsule:
            AnyShape(Capsule())
        case .circle:
            AnyShape(Circle())
        case .ellipse:
            AnyShape(Ellipse())
        default:
            AnyShape(Rectangle())
        }
    }

    /// Resolves `.auto` to `.light` or `.dark` based on the system color scheme.
    private var resolvedAppearance: FacetAppearance {
        if style.appearance == .auto {
            return colorScheme == .dark ? .dark : .light
        }
        return style.appearance
    }

    private var effectiveBackdropColor: SIMD3<Float> {
        if isGooParticipant, let color = gooBackdropColorMap[childID] { return color }
        return backdropColor
    }

    func body(content: Content) -> some View {
        if isGooParticipant {
            // Report this view's frame to the parent ConfluenceScope.
            // The scope renders the merged glass — this view renders nothing.
            gooParticipantBody(content: content)
        } else {
            // Standalone: render glass directly.
            standaloneBody(content: content)
        }
    }

    // MARK: - Goo Participant Path

    @ViewBuilder
    private func gooParticipantBody(content: Content) -> some View {
        // Build the stable view tree using base style — this is constructed
        // once and NOT rebuilt every frame. Interaction style mods are applied
        // by the Metal renderer via patchedConfluenceGroups; the SwiftUI foreground
        // uses the base style which is sufficient (zone AF colors adapt
        // to the backdrop which already reflects interaction positions).
        let styledContent = content.crystalHoverCapture(style: style)

        // Wrap in a per-child TimelineView that reads interaction offsets
        // from the static store. This is the ONLY view that re-evaluates
        // every frame during interactions — and its body is trivially cheap
        // (two dictionary lookups + .offset + .zIndex).
        GooInteractionApplicator(childID: childID) {
            Group {
                // Goo participants: apply foreground from appearance.
                // The contrast layer (dark shadow / light wash) is applied in the
                // Metal shader where the merged SDF membrane shape is known per-pixel.
                if resolvedAppearance == .dark {
                    styledContent
                        .foregroundStyle(.white, .white.opacity(0.6), .white.opacity(0.4))
                } else if resolvedAppearance == .light {
                    styledContent
                        .foregroundStyle(.black, .black.opacity(0.6), .black.opacity(0.4))
                } else {
                    styledContent  // .base, .auto, and custom appearances
                }
            }
            .environment(\.gleamMaskTexture, gooLuminanceMaskTexture)
            .environment(\.facetFrame, confluenceScopeFrame)
            .environment(\.gleamMaskGeneration, gooLuminanceMaskGeneration)
            .environment(\.gleamChildID, childID)
            .background {
                EquatableView(content: GooChildFrameReporter(
                    childID: childID,
                    style: style,
                    shape: shape,
                    crystallized: crystalCrystallized || facetScopeActive
                ))
            }
            .onDisappear { ConfluenceLiveChildStore.remove(childID) }
        }
    }

    // MARK: - Standalone Path

    @ViewBuilder
    private func standaloneBody(content: Content) -> some View {
        let styledContent = content.crystalHoverCapture(style: style)

        Group {
            if resolvedAppearance == .dark {
                styledContent
                    .foregroundStyle(.white, .white.opacity(0.6), .white.opacity(0.4))
            } else if resolvedAppearance == .light {
                styledContent
                    .foregroundStyle(.black, .black.opacity(0.6), .black.opacity(0.4))
            } else {
                // .base, .auto, and custom appearances — default SwiftUI foreground.
                styledContent
            }
        }
        .background {
            // Contrast layer between glass and content, clipped to shape.
            // Dark: inverted mask darkens bright areas (shadow layer).
            // Light: inverted mask brightens dark areas (additive blend).
            if resolvedAppearance == .dark {
                GleamMaskView(texture: luminanceMaskTexture, generation: luminanceMaskGeneration)
                    .colorInvert()
                    .opacity(0.5)
                    .clipShape(glassClipShape)
                    .allowsHitTesting(false)
            } else if resolvedAppearance == .light {
                Color.white
                    .opacity(0.35)
                    .clipShape(glassClipShape)
                    .allowsHitTesting(false)
            }
        }
        .environment(\.facetLuminance, backgroundLuminance)
        .environment(\.gleamMaskTexture, luminanceMaskTexture)
        .environment(\.facetFrame, glassFrame)
        .environment(\.gleamMaskGeneration, luminanceMaskGeneration)
        .environment(\.gleamChildID, childID)
        .background {
            GeometryReader { geo in
                SettingRepresentable(
                    style: style,
                    shape: shape,
                    gazePoint: gazePoint,
                    backgroundLuminance: $backgroundLuminance,
                    backdropColor: $backdropColor,
                    luminanceMaskTexture: $luminanceMaskTexture,
                    luminanceMaskGeneration: $luminanceMaskGeneration
                )
                .allowsHitTesting(false)
                .onAppear { glassFrame = geo.frame(in: .global) }
                .onChange(of: geo.frame(in: .global)) { _, newFrame in
                    glassFrame = newFrame
                }
            }
        }
    }
}

// MARK: - Goo Interaction Applicator

/// Lightweight per-child wrapper that applies interaction offsets from the
/// static ``ConfluenceLiveChildStore`` every animation frame.
///
/// When interactions are inactive (`interactionsActive == false`), this
/// renders the content directly with no overhead — no `TimelineView`.
/// When active, a `TimelineView(.animation)` drives per-frame reads of
/// the offset and z-index dictionaries, applying `.offset()` and `.zIndex()`.
///
/// The content view tree is built ONCE by the caller and passed in. The
/// `TimelineView` body is trivially cheap: two dictionary lookups + modifiers.
/// This replaces the old approach of writing per-child offsets through SwiftUI
/// environment, which caused every child's modifier body to re-evaluate with
/// its full view tree (foreground, mask, compositing) every frame.
private struct GooInteractionApplicator<Content: View>: View {
    let childID: String
    @ViewBuilder let content: Content

    var body: some View {
        if ConfluenceLiveChildStore.interactionsActive {
            TimelineView(.animation) { _ in
                let offset = ConfluenceLiveChildStore.interactionOffsets[childID] ?? .zero
                let zIndex = ConfluenceLiveChildStore.interactionZIndices[childID] ?? 0
                content
                    .offset(offset)
                    .zIndex(zIndex)
            }
        } else {
            content
        }
    }
}

// MARK: - Goo Frame Reporter

/// Reports a goo child's frame via preference and live child store.
///
/// Extracted into its own `View` so that SwiftUI can skip re-evaluating
/// the GeometryReader + preference + onChange when only environment
/// values change on the parent modifier. This breaks the cascade:
/// environment change → body re-eval → preference/onChange fire →
/// "tried to update multiple times per frame".
///
/// Conforms to `Equatable` so SwiftUI uses `==` to decide whether to
/// re-evaluate the body. No closures — closures defeat Equatable diffing.
private struct GooChildFrameReporter: View, Equatable {
    let childID: String
    let style: FacetStyle
    let shape: ShapeDescriptor
    let crystallized: Bool

    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.childID == rhs.childID
            && lhs.style == rhs.style
            && lhs.crystallized == rhs.crystallized
    }

    var body: some View {
        GeometryReader { geo in
            #if os(iOS) || os(visionOS)
            let reportedFrame = geo.frame(in: .global)
            #else
            let reportedFrame = geo.frame(in: .named("confluenceScope"))
            #endif
            Color.clear
                .onChange(of: reportedFrame) { _, newFrame in
                    ConfluenceLiveChildStore.update(ConfluenceChildInfo(
                        id: childID,
                        frame: newFrame,
                        style: style,
                        shape: shape,
                        crystallized: crystallized
                    ))
                }
                .preference(
                    key: ConfluenceChildrenKey.self,
                    value: [ConfluenceChildInfo(
                        id: childID,
                        frame: reportedFrame,
                        style: style,
                        shape: shape,
                        crystallized: crystallized
                    )]
                )
        }
    }
}

// MARK: - View Extensions

extension View {

    /// Applies a Liquid Glass effect with the default `.regular` style and rounded rectangle shape.
    public func facet() -> some View {
        modifier(FacetModifier(style: .regular, shape: .roundedRect()))
    }

    /// Applies a Liquid Glass effect with the given style and rounded rectangle shape.
    public func facet(_ style: FacetStyle) -> some View {
        modifier(FacetModifier(style: style, shape: .roundedRect()))
    }

    /// Applies a Liquid Glass effect with the given style and a `RoundedRectangle` clip shape.
    public func facet(
        _ style: FacetStyle = .regular,
        in shape: RoundedRectangle
    ) -> some View {
        let radius = shape.cornerSize.width
        return modifier(FacetModifier(
            style: style,
            shape: .roundedRect(cornerRadius: radius)
        ))
    }

    /// Applies a Liquid Glass effect with the given style and a `Capsule` clip shape.
    public func facet(
        _ style: FacetStyle = .regular,
        in shape: Capsule
    ) -> some View {
        modifier(FacetModifier(style: style, shape: .capsule))
    }

    /// Applies a Liquid Glass effect with the given style and a `Circle` clip shape.
    public func facet(
        _ style: FacetStyle = .regular,
        in shape: Circle
    ) -> some View {
        modifier(FacetModifier(style: style, shape: .circle))
    }

    /// Applies a Liquid Glass effect with the given style and an `Ellipse` clip shape.
    public func facet(
        _ style: FacetStyle = .regular,
        in shape: Ellipse
    ) -> some View {
        modifier(FacetModifier(style: style, shape: .ellipse))
    }

    /// Applies a Liquid Glass effect with a custom `ShapeDescriptor`.
    ///
    /// Use this for advanced shapes like polygons, stars, or custom SDF textures:
    /// ```swift
    /// .facet(.regular, shape: .polygon(sides: 6, cornerRadius: 4))
    /// .facet(.regular, shape: .star(points: 5, innerRadius: 0.4))
    /// ```
    public func facet(
        _ style: FacetStyle = .regular,
        shape: ShapeDescriptor
    ) -> some View {
        modifier(FacetModifier(style: style, shape: shape))
    }

    /// Provides an external light source position to all CrystalKit glass views in this subtree.
    ///
    /// On iPhone, CrystalKit automatically tracks device tilt for light direction.
    /// On iPad, Apple Pencil hover / trackpad cursor drives the light.
    /// On macOS, mouse cursor drives the light.
    ///
    /// Use this modifier only when you have a custom input source:
    /// ```swift
    /// MyView()
    ///     .gleamSource(customPosition)
    /// ```
    public func gleamSource(_ point: CGPoint?) -> some View {
        environment(\.gleamPoint, point)
    }

    /// Legacy alias for ``gleamSource(_:)``.
    public func gleamPoint(_ point: CGPoint?) -> some View {
        environment(\.gleamPoint, point)
    }

    /// Tells standalone glass views about a SwiftUI-level position offset
    /// so their UV / refraction coordinates stay in sync with the visual position.
    ///
    /// Use this when you move glass with `.offset()` — the Metal NSView doesn't
    /// see SwiftUI offset transforms, so without this the refraction stays anchored
    /// to the original position.
    public func facetOffset(_ offset: CGSize) -> some View {
        environment(\.facetOffset, offset)
    }

    /// Moves a glass view and keeps its refraction in sync.
    ///
    /// Combines `.facetOffset()` (tells Metal about the offset) and `.offset()`
    /// (visually moves the view) in one call:
    /// ```swift
    /// Circle()
    ///     .facet(.regular, in: Circle())
    ///     .facetMovable(offset: dragOffset)
    /// ```
    public func facetMovable(offset: CGSize) -> some View {
        self
            .facetOffset(offset)
            .offset(x: offset.width, y: offset.height)
    }

    /// Enables Apple Pencil / trackpad hover tracking for CrystalKit glass effects.
    ///
    /// Apply this to a container view that spans the area where you want
    /// hover-based lighting to respond. Typically your root content view:
    ///
    /// ```swift
    /// struct ContentView: View {
    ///     var body: some View {
    ///         ZStack {
    ///             MyBackground()
    ///             MyGlassViews()
    ///         }
    ///         .gleamTracking()
    ///     }
    /// }
    /// ```
    ///
    /// On iPad, this captures Apple Pencil and trackpad hover events and feeds
    /// them to all glass views in the subtree. On iPhone and macOS this is a no-op
    /// (iPhone uses tilt, macOS uses mouse monitoring built into each view).
    @ViewBuilder
    public func gleamTracking() -> some View {
        #if os(iOS)
        self.overlay {
            if UIDevice.current.userInterfaceIdiom == .pad {
                CrystalHoverOverlay()
            }
        }
        #else
        self
        #endif
    }
}

// MARK: - Role-Based Modifier

/// Resolves a `FacetRole` against the environment's `FacetStylesheet`
/// and delegates to the standard `FacetModifier`.
struct FacetRoleModifier: ViewModifier {
    let role: FacetRole
    let shape: ShapeDescriptor

    @Environment(FacetStylesheet.self) private var stylesheet: FacetStylesheet?

    private var resolvedStyle: FacetStyle {
        let sheet = stylesheet ?? FacetStylesheet.default
        return sheet.style(for: role)
    }

    func body(content: Content) -> some View {
        content.modifier(FacetModifier(style: resolvedStyle, shape: shape))
    }
}

// MARK: - Role-Based View Extensions

extension View {
    /// Applies a Liquid Glass effect for the given semantic role with a rounded rectangle shape.
    public func facet(_ role: FacetRole) -> some View {
        modifier(FacetRoleModifier(role: role, shape: .roundedRect()))
    }

    /// Applies a Liquid Glass effect for the given semantic role with a `RoundedRectangle` clip shape.
    public func facet(
        _ role: FacetRole,
        in shape: RoundedRectangle
    ) -> some View {
        let radius = shape.cornerSize.width
        return modifier(FacetRoleModifier(
            role: role,
            shape: .roundedRect(cornerRadius: radius)
        ))
    }

    /// Applies a Liquid Glass effect for the given semantic role with a `Capsule` clip shape.
    public func facet(
        _ role: FacetRole,
        in shape: Capsule
    ) -> some View {
        modifier(FacetRoleModifier(role: role, shape: .capsule))
    }

    /// Applies a Liquid Glass effect for the given semantic role with a `Circle` clip shape.
    public func facet(
        _ role: FacetRole,
        in shape: Circle
    ) -> some View {
        modifier(FacetRoleModifier(role: role, shape: .circle))
    }

    /// Applies a Liquid Glass effect for the given semantic role with an `Ellipse` clip shape.
    public func facet(
        _ role: FacetRole,
        in shape: Ellipse
    ) -> some View {
        modifier(FacetRoleModifier(role: role, shape: .ellipse))
    }

    /// Applies a Liquid Glass effect for the given semantic role with a custom `ShapeDescriptor`.
    public func facet(
        _ role: FacetRole,
        shape: ShapeDescriptor
    ) -> some View {
        modifier(FacetRoleModifier(role: role, shape: shape))
    }
}

// MARK: - Per-View Hover Capture (iPad)

/// Internal modifier applied to glass content views on iPad.
/// Captures hover events on the interactive content itself — acts as
/// a guaranteed fallback for when the full-screen overlay doesn't fire.
private struct CrystalHoverCaptureModifier: ViewModifier {
    let style: FacetStyle

    func body(content: Content) -> some View {
        #if os(iOS)
        if case .cursor = style.lightSource {
            content
                .onContinuousHover(coordinateSpace: .global) { phase in
                    switch phase {
                    case .active(let location):
                        GleamHoverTracker._sharedHoverPoint = location
                    case .ended:
                        GleamHoverTracker._sharedHoverPoint = nil
                    }
                }
        } else {
            content
        }
        #else
        content
        #endif
    }
}

extension View {
    /// Internal: attaches hover capture on iPad for cursor-mode glass.
    func crystalHoverCapture(style: FacetStyle) -> some View {
        modifier(CrystalHoverCaptureModifier(style: style))
    }
}

// MARK: - Hover Overlay (iPad)

#if os(iOS)
import UIKit

/// Transparent overlay that captures Apple Pencil / trackpad hover events
/// without blocking touch events.
///
/// Uses a UIKit view that:
/// - Has `isUserInteractionEnabled = true` (receives hover events)
/// - Returns `nil` from `hitTest(_:with:)` for all touches (passes them through)
/// - Attaches a `UIHoverGestureRecognizer` that writes positions to the shared tracker
private struct CrystalHoverOverlay: UIViewRepresentable {
    func makeUIView(context: Context) -> HoverPassthroughView {
        let view = HoverPassthroughView()
        let hover = UIHoverGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleHover(_:)))
        view.addGestureRecognizer(hover)
        return view
    }

    func updateUIView(_ uiView: HoverPassthroughView, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator() }

    @MainActor final class Coordinator: NSObject {
        @objc func handleHover(_ recognizer: UIHoverGestureRecognizer) {
            guard let view = recognizer.view, let window = view.window else { return }
            switch recognizer.state {
            case .began, .changed:
                let locationInWindow = recognizer.location(in: window)
                let screenPt = window.convert(locationInWindow, to: nil)
                GleamHoverTracker._sharedHoverPoint = screenPt
            case .ended, .cancelled:
                GleamHoverTracker._sharedHoverPoint = nil
            default:
                break
            }
        }
    }
}

/// A UIView that captures hover events but passes all touches through.
///
/// `hitTest(_:with:)` always returns `nil`, making this view invisible to
/// the touch system. But `UIHoverGestureRecognizer` uses the pointer event
/// system (not touch hit testing) and still fires on this view.
final class HoverPassthroughView: UIView {
    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        isUserInteractionEnabled = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // Pass all touches through — only hover events should reach
        // the gesture recognizer.
        return nil
    }
}
#endif
