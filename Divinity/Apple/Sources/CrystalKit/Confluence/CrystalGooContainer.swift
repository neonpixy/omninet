// ConfluenceContainer.swift
// CrystalKit
//
// Seamless liquid glass merging between Crystal Glass views.
// Chain `.confluence()` onto any `.facet()` view to opt it in.
// Nearby opted-in views merge organically into one continuous glass surface.
//
// Usage:
//   ConfluenceScope(radius: 80) {
//       Circle()
//           .frame(width: 60, height: 60)
//           .facet()
//           .confluence()
//
//       RoundedRectangle(cornerRadius: 12)
//           .frame(width: 100, height: 50)
//           .facet(.frosted)
//           .confluence()
//
//       Text("Not goo")
//           .facet()  // no .confluence() — renders standalone
//   }

import SwiftUI
import Metal

// MARK: - Goo Scope

/// Defines a region where `.confluence()` views can merge with each other.
///
/// Each group of nearby opted-in views is rendered as a single unified glass
/// surface using one MTKView per group — no compositing seams.
///
/// ```swift
/// ConfluenceScope(radius: 80) {
///     HStack(spacing: 20) {
///         icon1.facet().confluence()
///         icon2.facet().confluence()
///         icon3.facet()  // standalone — no goo
///     }
/// }
/// ```
public struct ConfluenceScope<Content: View>: View {

    /// Distance in points at which two goo views start merging.
    public var radius: CGFloat

    /// Smooth-min blending radius in canvas points. Controls how wide the
    /// bridge between merging shapes is. Default 40.
    public var smoothK: Float

    /// The glass style used for the merged surface. Defaults to `.regular`.
    public var style: FacetStyle

    /// Interaction effects evaluated pairwise for children within range.
    /// Empty = no physics overhead (no TimelineView created).
    public var effects: [AnyResonanceEffect]

    let content: Content

    @State private var children: [ConfluenceChildInfo] = []
    @State private var luminanceMap: [String: CGFloat] = [:]
    @State private var backdropColorMap: [String: SIMD3<Float>] = [:]
    @State private var scopeSize: CGSize = .zero
    @State private var scopeCenter: CGPoint = .zero
    @State private var scopeFrame: CGRect = .zero
    @State private var interactionEngine = ResonanceEngine()
    @State private var gooLuminanceMaskTexture: MTLTexture?
    @State private var gooLuminanceMaskGeneration: UInt64 = 0
    @State private var afProbeResults: [String: CGFloat] = [:]
    @Environment(\.gleamPoint) private var gazePoint

    public init(
        radius: CGFloat = 80,
        smoothK: Float = 40,
        style: FacetStyle = .regular,
        effects: [AnyResonanceEffect] = [],
        @ViewBuilder content: () -> Content
    ) {
        self.radius = radius
        self.smoothK = smoothK
        self.style = style
        self.effects = effects
        self.content = content()
    }

    public var body: some View {
        if effects.isEmpty {
            let _ = { ConfluenceLiveChildStore.interactionsActive = false }()
            scopeContent
        } else {
            let _ = { ConfluenceLiveChildStore.interactionsActive = true }()
            TimelineView(.animation) { timeline in
                // Read latest positions from the live child store — it's
                // updated during layout (onChange) so it's current-frame.
                // The @State `children` come through preferences, which
                // are 1-2 frames late. We still use `children` to know
                // WHICH IDs belong to this scope, then look up their
                // latest frames from the live store.
                let liveChildren = children.map { child in
                    ConfluenceLiveChildStore.children[child.id] ?? child
                }
                let _ = interactionEngine.tick(
                    children: liveChildren,
                    effects: effects,
                    scopeRadius: radius,
                    now: timeline.date
                )
                // Write interaction data to static store — children read via
                // per-child TimelineViews, NOT SwiftUI environment. This avoids
                // re-evaluating every child modifier body every frame.
                let _ = {
                    ConfluenceLiveChildStore.interactionOffsets = interactionEngine.offsetMap
                    ConfluenceLiveChildStore.interactionZIndices = interactionEngine.zIndexMap
                    ConfluenceLiveChildStore.interactionStyleMods = interactionEngine.styleModMap
                }()
                scopeContent
            }
        }
    }

    @ViewBuilder
    private var scopeContent: some View {
        ZStack {
            // Force the ZStack to fill all available space so the MTKView
            // covers the full draggable area, not just the content bounds.
            Color.clear

            // Single Metal view for all proximity groups.
            // Groups are rendered sequentially into one drawable each frame,
            // so the view is never destroyed when groups split or merge — no flicker.
            // Goo output texture is shared via ConfluenceOutputStore (static),
            // not via environment, so standalone glass siblings can read it too.
            ConfluenceRepresentable(
                groups: patchedConfluenceGroups,
                scopeSize: scopeSize,
                style: style,
                smoothK: smoothK,
                gazePoint: gazePoint,
                onLuminanceUpdate: { childIDs, lum in
                    // Suppress during interactions — the TimelineView already
                    // drives rendering, and updating @State here would cascade:
                    // render → callback → @State → environment → body → render.
                    guard !ConfluenceLiveChildStore.interactionsActive else { return }
                    for id in childIDs {
                        luminanceMap[id] = lum
                    }
                },
                onBackdropColorUpdate: { childIDs, color in
                    guard !ConfluenceLiveChildStore.interactionsActive else { return }
                    for id in childIDs {
                        backdropColorMap[id] = color
                    }
                },
                onLuminanceMaskUpdate: { tex in
                    gooLuminanceMaskTexture = tex
                    gooLuminanceMaskGeneration &+= 1
                },
                onAFProbeResults: { results in
                    afProbeResults = results
                }
            )
            .allowsHitTesting(false)

            content
                .environment(\.confluenceScopeActive, true)
                .environment(\.confluenceLuminanceMap, luminanceMap)
                .environment(\.confluenceBackdropColorMap, backdropColorMap)
                .environment(\.confluenceScopeCenter, scopeCenter)
                .environment(\.confluenceLuminanceMaskTexture, gooLuminanceMaskTexture)
                .environment(\.confluenceLuminanceMaskGeneration, gooLuminanceMaskGeneration)
                .environment(\.confluenceScopeSize, scopeSize)
                .environment(\.confluenceScopeFrame, scopeFrame)
                .environment(\.gleamProbeResults, afProbeResults)
        }
        .coordinateSpace(name: "confluenceScope")
        .onPreferenceChange(ConfluenceChildrenKey.self) { newChildren in
            // Only update @State when children are added/removed/changed identity.
            // Frame-only changes must NOT trigger a state update — the live child
            // store handles real-time frames. Updating @State here would re-evaluate
            // the scope body, which re-ticks the TimelineView, creating a cascade.
            let newIDs = Set(newChildren.map(\.id))
            let oldIDs = Set(children.map(\.id))
            let stylesChanged = !newChildren.allSatisfy { new in
                children.contains { $0.id == new.id && $0.style == new.style && $0.crystallized == new.crystallized }
            }
            if newIDs != oldIDs || stylesChanged || children.isEmpty {
                children = newChildren
            }
        }
        .background {
            GeometryReader { geo in
                Color.clear
                    .onAppear {
                        scopeSize = geo.size
                        let global = geo.frame(in: .global)
                        scopeCenter = CGPoint(x: global.midX, y: global.midY)
                        scopeFrame = global
                    }
                    .onChange(of: geo.frame(in: .global)) { _, newGlobal in
                        scopeSize = newGlobal.size
                        scopeCenter = CGPoint(x: newGlobal.midX, y: newGlobal.midY)
                        scopeFrame = newGlobal
                    }
            }
        }
    }

    /// Goo groups with interaction style modifications and live frames applied.
    private var patchedConfluenceGroups: [ConfluenceGroup] {
        // Use live frames from the store (updated during layout/onChange) rather
        // than the @State `children` which come through preferences and are 1-2
        // frames stale. Live frames include interaction offsets because
        // GooInteractionApplicator uses .offset() which affects GeometryReader.
        let liveChildren = children.map { child in
            ConfluenceLiveChildStore.children[child.id] ?? child
        }
        let patchedChildren: [ConfluenceChildInfo]
        if effects.isEmpty {
            // Assign fallback z-index from declaration order so crystallized
            // children have distinct sort keys even without interaction effects.
            patchedChildren = liveChildren.enumerated().map { index, child in
                var c = child
                c.zIndex = Double(index)
                return c
            }
        } else {
            patchedChildren = liveChildren.enumerated().map { index, child in
                var c = child
                c.zIndex = interactionEngine.zIndexMap[child.id] ?? Double(index)
                if let mod = interactionEngine.styleModMap[child.id] {
                    c = ConfluenceChildInfo(
                        id: c.id,
                        frame: c.frame,
                        style: c.style.applying(mod),
                        shape: c.shape,
                        crystallized: c.crystallized,
                        zIndex: c.zIndex
                    )
                }
                return c
            }
        }
        return buildConfluenceGroups(from: patchedChildren, radius: radius)
    }
}

// MARK: - View Extension

extension View {
    /// Opts this Crystal Glass view into seamless merging with nearby glass views.
    ///
    /// Must be inside a ``ConfluenceScope`` and chained after `.facet()`.
    public func confluence() -> some View {
        modifier(ConfluenceModifier())
    }

    /// Opts this goo child out of merging while keeping it in the goo pipeline.
    ///
    /// The child renders through the shared Metal view (jitter-free) but never
    /// blends with neighboring shapes. Its own ``FacetStyle`` is used
    /// for all visual parameters.
    ///
    /// Must be chained after `.confluence()`:
    /// ```swift
    /// icon.facet(.frosted)
    ///     .confluence()
    ///     .crystallize()
    /// ```
    public func crystallize() -> some View {
        environment(\.confluenceCrystallized, true)
    }

    /// Tells the goo scope that this participant has been moved by the given offset.
    ///
    /// SwiftUI's `.offset()` modifier doesn't affect `GeometryReader.frame(in:)`,
    /// so draggable goo shapes need this to keep the Metal viewport in sync.
    /// Apply **before** `.offset()` — the glass modifier reads it via environment.
    ///
    /// ```swift
    /// myView
    ///     .facet(.regular, in: Circle())
    ///     .confluence()
    ///     .confluenceOffset(dragOffset)
    ///     .offset(dragOffset)
    /// ```
    public func confluenceOffset(_ offset: CGSize) -> some View {
        environment(\.confluenceOffset, offset)
    }
}

// MARK: - Crystal Scope

/// A jitter-free rendering scope for glass views.
///
/// `FacetScope` wraps its content in a shared Metal rendering view so that
/// every ``facet(_:in:)-`` child is rendered in a single compositing
/// pass with preference-based position tracking. This eliminates the
/// one-frame lag that causes jitter during scroll and drag with standalone
/// glass views.
///
/// Unlike ``ConfluenceScope``, children inside a `FacetScope` never merge —
/// each glass panel renders independently. No `.confluence()` or
/// `.crystallize()` modifiers are needed.
///
/// ```swift
/// FacetScope {
///     ScrollView {
///         ForEach(items) { item in
///             Card(item)
///                 .facet(style, in: RoundedRectangle(cornerRadius: 12))
///         }
///     }
/// }
/// .frame(maxWidth: .infinity, maxHeight: .infinity)
/// ```
///
/// > Important: The scope must fill the area behind its glass children for
/// > backdrop UV alignment. Use `.frame(maxWidth:maxHeight:)` to expand it.
public struct FacetScope<Content: View>: View {
    let content: Content
    let effects: [AnyResonanceEffect]

    public init(
        effects: [AnyResonanceEffect] = [],
        @ViewBuilder content: () -> Content
    ) {
        self.effects = effects
        self.content = content()
    }

    public var body: some View {
        ConfluenceScope(effects: effects) {
            content
                .environment(\.facetScopeActive, true)
        }
    }
}

// MARK: - Goo Opt-In Modifier

struct ConfluenceModifier: ViewModifier {
    func body(content: Content) -> some View {
        content.environment(\.confluenceEnabled, true)
    }
}

// MARK: - Goo Group

/// A proximity-connected set of goo children, rendered as one glass surface.
struct ConfluenceGroup: Identifiable, Equatable {
    let id: String
    let childIDs: [String]
    let children: [ConfluenceChildInfo]
    /// Bounding box in the scope's coordinate space (expanded by merge radius).
    let boundingBox: CGRect

    /// Lowest z-index among the group's children, used for back-to-front render ordering.
    var sortKey: Double { children.map(\.zIndex).min() ?? 0 }
}

// MARK: - Proximity Grouping

/// Groups children by proximity using union-find.
/// Two children are connected if their frames expanded by `radius` overlap.
func buildConfluenceGroups(from children: [ConfluenceChildInfo], radius: CGFloat) -> [ConfluenceGroup] {
    guard !children.isEmpty else { return [] }

    let n = children.count
    var parent = Array(0..<n)
    var rank = Array(repeating: 0, count: n)

    func find(_ x: Int) -> Int {
        var x = x
        while parent[x] != x {
            parent[x] = parent[parent[x]]
            x = parent[x]
        }
        return x
    }

    func union(_ a: Int, _ b: Int) {
        let ra = find(a), rb = find(b)
        guard ra != rb else { return }
        if rank[ra] < rank[rb] { parent[ra] = rb }
        else if rank[ra] > rank[rb] { parent[rb] = ra }
        else { parent[rb] = ra; rank[ra] += 1 }
    }

    for i in 0..<n {
        let expA = children[i].frame.insetBy(dx: -radius, dy: -radius)
        for j in (i + 1)..<n {
            let expB = children[j].frame.insetBy(dx: -radius, dy: -radius)
            if expA.intersects(expB)
                && !children[i].crystallized
                && !children[j].crystallized { union(i, j) }
        }
    }

    var components: [Int: [Int]] = [:]
    for i in 0..<n { components[find(i), default: []].append(i) }

    return components.values.map { indices in
        let groupChildren = indices.map { children[$0] }

        var bbox = groupChildren[0].frame
        for child in groupChildren.dropFirst() { bbox = bbox.union(child.frame) }
        bbox = bbox.insetBy(dx: -radius, dy: -radius)

        return ConfluenceGroup(
            id: groupChildren.map(\.id).sorted().joined(separator: "+"),
            childIDs: groupChildren.map(\.id),
            children: groupChildren,
            boundingBox: bbox
        )
    }
    .sorted { ($0.sortKey, $0.id) < ($1.sortKey, $1.id) }  // back-to-front, stable
}
