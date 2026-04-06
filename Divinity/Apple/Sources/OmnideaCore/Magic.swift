import COmnideaFFI
import Foundation
import os

private let logger = Logger(subsystem: "co.omnidea", category: "Magic")

// MARK: - MagicDocumentState

/// Swift wrapper around the Rust DocumentState (Magic's single source of truth).
///
/// Holds the digit tree, layout, and selection for a single .idea document.
/// All mutation methods return the generated operation as JSON for CRDT sync.
///
/// ```swift
/// let doc = MagicDocumentState(author: "alice")
/// let opJSON = doc.insert(digitJSON: digitStr, parentId: nil)
/// ```
public final class MagicDocumentState: @unchecked Sendable {
    let ptr: OpaquePointer

    public init(author: String) {
        ptr = divi_magic_document_new(author)!
    }

    deinit {
        divi_magic_document_free(ptr)
    }

    // MARK: - Querying

    /// The number of digits in the document.
    public var digitCount: Int {
        Int(divi_magic_document_digit_count(ptr))
    }

    /// The root digit's UUID string, or nil if the document has no root.
    public var rootId: String? {
        guard let cstr = divi_magic_document_root_id(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a single digit as JSON by UUID string. Returns nil if not found.
    public func digit(id: String) -> String? {
        guard let cstr = divi_magic_document_digit(ptr, id) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All digits in the document as a JSON array.
    public var allDigits: String {
        let cstr = divi_magic_document_all_digits(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Children of a digit as a JSON array.
    public func childrenOf(parentId: String) -> String {
        let cstr = divi_magic_document_children_of(ptr, parentId)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The document layout as JSON.
    public var layout: String {
        let cstr = divi_magic_document_layout(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The document selection state as JSON.
    public var selection: String {
        let cstr = divi_magic_document_selection(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The current author identifier.
    public var author: String {
        let cstr = divi_magic_document_author(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Modification

    /// Insert a digit into the document.
    ///
    /// - Parameters:
    ///   - digitJSON: The digit to insert, serialized as JSON.
    ///   - parentId: Optional parent digit UUID. Pass nil for root-level insert.
    /// - Returns: The generated operation as JSON, or nil on error.
    public func insert(digitJSON: String, parentId: String?) -> String? {
        guard let cstr = divi_magic_document_insert(ptr, digitJSON, parentId) else {
            logger.error("insert failed — check divi_last_error()")
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Update a field on a digit.
    ///
    /// - Parameters:
    ///   - id: The digit UUID string.
    ///   - field: The field name to update.
    ///   - oldJSON: The old value as JSON (for conflict detection).
    ///   - newJSON: The new value as JSON.
    /// - Returns: The generated operation as JSON, or nil on error.
    public func update(id: String, field: String, oldJSON: String, newJSON: String) -> String? {
        guard let cstr = divi_magic_document_update(ptr, id, field, oldJSON, newJSON) else {
            logger.error("update failed — check divi_last_error()")
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Delete a digit (tombstone soft-delete).
    ///
    /// - Parameter id: The digit UUID string.
    /// - Returns: The generated operation as JSON, or nil on error.
    public func delete(id: String) -> String? {
        guard let cstr = divi_magic_document_delete(ptr, id) else {
            logger.error("delete failed — check divi_last_error()")
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Apply a DigitOperation (from JSON) to the document.
    ///
    /// - Parameter operationJSON: The operation to apply, serialized as JSON.
    /// - Returns: 0 if applied, 1 if duplicate (already applied), -1 on error.
    public func apply(operationJSON: String) -> Int32 {
        divi_magic_document_apply(ptr, operationJSON)
    }

    /// Replace the document's digit set wholesale.
    ///
    /// - Parameters:
    ///   - digitsJSON: A JSON array of digits.
    ///   - rootId: Optional root digit UUID string.
    /// - Returns: 0 on success, -1 on error.
    @discardableResult
    public func loadDigits(digitsJSON: String, rootId: String?) -> Int32 {
        divi_magic_document_load_digits(ptr, digitsJSON, rootId)
    }

    /// Set the document layout from JSON.
    ///
    /// - Parameter layoutJSON: The layout to set, serialized as JSON.
    /// - Returns: 0 on success, -1 on error.
    @discardableResult
    public func setLayout(layoutJSON: String) -> Int32 {
        divi_magic_document_set_layout(ptr, layoutJSON)
    }

    // MARK: - Selection

    /// Select a digit in the document's selection state.
    public func select(id: String) {
        divi_magic_document_select(ptr, id)
    }

    /// Deselect a digit from the document's selection state.
    public func deselectDigit(id: String) {
        divi_magic_document_deselect_digit(ptr, id)
    }

    /// Clear all selection in the document.
    public func clearSelection() {
        divi_magic_document_clear_selection(ptr)
    }
}

// MARK: - MagicCanvasState

/// Swift wrapper around the Rust CanvasState (viewport, zoom, selection, snapping).
///
/// Manages the canvas viewport transformation, zoom level, grid snapping,
/// and canvas-level selection (distinct from document selection).
///
/// ```swift
/// let canvas = MagicCanvasState(width: 1920, height: 1080)
/// let (cx, cy) = canvas.screenToCanvas(sx: 100, sy: 200)
/// canvas.zoomBy(factor: 1.5, centerX: cx, centerY: cy)
/// ```
public final class MagicCanvasState: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a canvas with the default viewport (1024x768).
    public init() {
        ptr = divi_magic_canvas_new()!
    }

    /// Create a canvas with a specific viewport size.
    public init(width: Double, height: Double) {
        ptr = divi_magic_canvas_with_viewport(width, height)!
    }

    deinit {
        divi_magic_canvas_free(ptr)
    }

    // MARK: - Coordinate Conversion

    /// Convert a screen-space point to canvas-space.
    public func screenToCanvas(sx: Double, sy: Double) -> (Double, Double) {
        var outCX: Double = 0
        var outCY: Double = 0
        divi_magic_canvas_screen_to_canvas(ptr, sx, sy, &outCX, &outCY)
        return (outCX, outCY)
    }

    /// Convert a canvas-space point to screen-space.
    public func canvasToScreen(cx: Double, cy: Double) -> (Double, Double) {
        var outSX: Double = 0
        var outSY: Double = 0
        divi_magic_canvas_canvas_to_screen(ptr, cx, cy, &outSX, &outSY)
        return (outSX, outSY)
    }

    // MARK: - Zoom

    /// The current zoom level.
    public var zoomLevel: Double {
        divi_magic_canvas_zoom_level(ptr)
    }

    /// Set the zoom level (clamped to valid range).
    public func setZoom(_ level: Double) {
        divi_magic_canvas_set_zoom(ptr, level)
    }

    /// Zoom by a multiplicative factor around a center point (canvas coords).
    public func zoomBy(factor: Double, centerX: Double, centerY: Double) {
        divi_magic_canvas_zoom_by(ptr, factor, centerX, centerY)
    }

    /// Zoom to fit a rectangle in the viewport.
    public func zoomToFit(x: Double, y: Double, w: Double, h: Double) {
        divi_magic_canvas_zoom_to_fit(ptr, x, y, w, h)
    }

    /// Set zoom to 100% (actual size).
    public func zoomActualSize() {
        divi_magic_canvas_zoom_actual_size(ptr)
    }

    /// Set zoom to a specific percentage (e.g. 200.0 for 200%).
    public func zoomPercent(_ percent: Double) {
        divi_magic_canvas_zoom_percent(ptr, percent)
    }

    // MARK: - Selection

    /// The current canvas selection as a JSON array of UUID strings.
    public func selection() -> String {
        let cstr = divi_magic_canvas_selection(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Select a single digit on the canvas (replaces current selection).
    public func select(id: String) {
        divi_magic_canvas_select(ptr, id)
    }

    /// Select multiple digits on the canvas (adds to current selection).
    public func selectMultiple(idsJSON: String) {
        divi_magic_canvas_select_multiple(ptr, idsJSON)
    }

    /// Deselect a digit on the canvas.
    public func deselect(id: String) {
        divi_magic_canvas_deselect(ptr, id)
    }

    /// Clear the canvas selection entirely.
    public func clearSelection() {
        divi_magic_canvas_clear_selection(ptr)
    }

    /// Check whether a digit is selected on the canvas.
    public func isSelected(id: String) -> Bool {
        divi_magic_canvas_is_selected(ptr, id)
    }

    /// The number of selected items on the canvas.
    public var selectionCount: Int {
        Int(divi_magic_canvas_selection_count(ptr))
    }

    // MARK: - Grid & Snapping

    /// Snap a point to the grid (if enabled).
    public func snapPoint(x: Double, y: Double) -> (Double, Double) {
        var outX: Double = 0
        var outY: Double = 0
        divi_magic_canvas_snap_point(ptr, x, y, &outX, &outY)
        return (outX, outY)
    }

    /// Set grid size and snap-to-grid behavior. Pass 0 for size to disable grid.
    public func setGrid(size: Double, snap: Bool) {
        divi_magic_canvas_set_grid(ptr, size, snap)
    }

    /// Set whether alignment guides are visible.
    public func setGuides(visible: Bool) {
        divi_magic_canvas_set_guides(ptr, visible)
    }

    // MARK: - Handles

    /// Compute drag handles for a selection bounding rect at the given zoom.
    ///
    /// Returns a JSON array of DragHandle objects.
    public static func computeHandles(
        x: Double, y: Double, w: Double, h: Double, zoom: Double
    ) -> String {
        let cstr = divi_magic_canvas_compute_handles(x, y, w, h, zoom)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Hit-test a point against a set of handles.
    ///
    /// - Parameters:
    ///   - px: Point x coordinate.
    ///   - py: Point y coordinate.
    ///   - handlesJSON: JSON array of DragHandle objects.
    /// - Returns: The matching DragHandle as JSON, or nil if no hit.
    public static func hitTestHandles(
        px: Double, py: Double, handlesJSON: String
    ) -> String? {
        guard let cstr = divi_magic_canvas_hit_test_handles(px, py, handlesJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - MagicRendererRegistry

/// Swift wrapper around the Rust RendererRegistry (Magic's Imagination layer).
///
/// Manages digit renderers that transform Digit data into RenderSpecs.
/// Includes a fallback renderer for unknown digit types.
///
/// ```swift
/// let registry = MagicRendererRegistry(withBuiltIns: true)
/// let specJSON = registry.render(
///     digitJSON: digit, modeJSON: mode, contextJSON: ctx
/// )
/// ```
public final class MagicRendererRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty renderer registry (with fallback renderer).
    public init() {
        ptr = divi_magic_renderer_registry_new()!
    }

    /// Create a renderer registry, optionally pre-loaded with all built-in renderers.
    public init(withBuiltIns: Bool) {
        if withBuiltIns {
            ptr = divi_magic_renderer_registry_new_with_all()!
        } else {
            ptr = divi_magic_renderer_registry_new()!
        }
    }

    deinit {
        divi_magic_renderer_registry_free(ptr)
    }

    // MARK: - Querying

    /// The number of registered renderers.
    public var count: Int {
        Int(divi_magic_renderer_registry_count(ptr))
    }

    /// Check whether a renderer is registered for the given digit type.
    public func has(digitType: String) -> Bool {
        divi_magic_renderer_registry_has(ptr, digitType)
    }

    /// The registered digit types as a JSON array of strings.
    public func types() -> String {
        let cstr = divi_magic_renderer_registry_types(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Rendering

    /// Render a digit using the appropriate renderer.
    ///
    /// - Parameters:
    ///   - digitJSON: The digit to render, serialized as JSON.
    ///   - modeJSON: The render mode, serialized as JSON.
    ///   - contextJSON: The render context, serialized as JSON.
    /// - Returns: The RenderSpec as JSON, or nil on error.
    public func render(
        digitJSON: String, modeJSON: String, contextJSON: String
    ) -> String? {
        guard let cstr = divi_magic_renderer_render(ptr, digitJSON, modeJSON, contextJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the estimated size for a digit without full rendering.
    ///
    /// - Parameters:
    ///   - digitJSON: The digit, serialized as JSON.
    ///   - contextJSON: The render context, serialized as JSON.
    /// - Returns: A tuple (width, height), or nil on error.
    public func estimatedSize(
        digitJSON: String, contextJSON: String
    ) -> (Double, Double)? {
        var outW: Double = 0
        var outH: Double = 0
        let result = divi_magic_renderer_estimated_size(
            ptr, digitJSON, contextJSON, &outW, &outH
        )
        guard result == 0 else { return nil }
        return (outW, outH)
    }
}

// MARK: - MagicRenderCache

/// Swift wrapper around the Rust RenderCache (LRU cache for RenderSpecs).
///
/// Caches rendered specs by digit ID and render mode to avoid redundant rendering.
///
/// ```swift
/// let cache = MagicRenderCache(maxSize: 500)
/// cache.insert(specJSON: specStr)
/// if let cached = cache.get(digitId: id, modeJSON: mode) { ... }
/// ```
public final class MagicRenderCache: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a render cache with the default max size (200).
    public init() {
        ptr = divi_magic_cache_new()!
    }

    /// Create a render cache with a specific max size.
    public init(maxSize: Int) {
        ptr = divi_magic_cache_with_max_size(UInt(maxSize))!
    }

    deinit {
        divi_magic_cache_free(ptr)
    }

    // MARK: - Cache Operations

    /// Get a cached RenderSpec as JSON. Returns nil on cache miss.
    ///
    /// - Parameters:
    ///   - digitId: The digit UUID string.
    ///   - modeJSON: The render mode, serialized as JSON.
    /// - Returns: The cached RenderSpec as JSON, or nil if not cached.
    public func get(digitId: String, modeJSON: String) -> String? {
        guard let cstr = divi_magic_cache_get(ptr, digitId, modeJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Insert a RenderSpec into the cache.
    ///
    /// - Parameter specJSON: The RenderSpec serialized as JSON.
    /// - Returns: 0 on success, -1 on error.
    @discardableResult
    public func insert(specJSON: String) -> Int32 {
        divi_magic_cache_insert(ptr, specJSON)
    }

    /// Invalidate all cached specs for a digit.
    public func invalidate(digitId: String) {
        divi_magic_cache_invalidate(ptr, digitId)
    }

    /// Clear the entire render cache.
    public func invalidateAll() {
        divi_magic_cache_invalidate_all(ptr)
    }

    /// The number of cached entries.
    public var size: Int {
        Int(divi_magic_cache_size(ptr))
    }

    /// The cache hit rate (0.0 to 1.0).
    public var hitRate: Double {
        divi_magic_cache_hit_rate(ptr)
    }
}

// MARK: - MagicToolRegistry

/// Swift wrapper around the Rust ToolRegistry (canvas tool management).
///
/// Manages interactive tools (select, draw, text, etc.) and dispatches
/// input events to the active tool. Tool event methods take both the tool
/// registry and a MagicDocumentState since tools read and modify the document.
///
/// ```swift
/// let tools = MagicToolRegistry(withDefaults: true)
/// tools.select(id: "select")
/// let actionJSON = tools.onPress(x: 100, y: 200, shift: false, alt: false, command: false, doc: doc)
/// ```
public final class MagicToolRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty tool registry.
    public init() {
        ptr = divi_magic_tool_registry_new()!
    }

    /// Create a tool registry, optionally pre-loaded with default tools.
    public init(withDefaults: Bool) {
        if withDefaults {
            ptr = divi_magic_tool_registry_new_default()!
        } else {
            ptr = divi_magic_tool_registry_new()!
        }
    }

    deinit {
        divi_magic_tool_registry_free(ptr)
    }

    // MARK: - Querying

    /// The number of registered tools.
    public var count: Int {
        Int(divi_magic_tool_registry_count(ptr))
    }

    /// The IDs of all registered tools as a JSON array of strings.
    public func list() -> String {
        let cstr = divi_magic_tool_registry_list(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Select (activate) a tool by ID.
    ///
    /// - Parameter id: The tool ID string.
    /// - Returns: `true` if the tool was found and activated.
    @discardableResult
    public func select(id: String) -> Bool {
        divi_magic_tool_registry_select(ptr, id)
    }

    /// The active tool's ID, or nil if no tool is active.
    public var activeId: String? {
        guard let cstr = divi_magic_tool_registry_active_id(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The active tool's cursor style as JSON, or nil if no tool is active.
    public var activeCursor: String? {
        guard let cstr = divi_magic_tool_registry_active_cursor(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Input Events

    /// Handle a press event on the active tool.
    ///
    /// - Returns: The resulting ToolAction as JSON, or nil on error.
    public func onPress(
        x: Double, y: Double,
        shift: Bool, alt: Bool, command: Bool,
        doc: MagicDocumentState
    ) -> String? {
        guard let cstr = divi_magic_tool_on_press(
            ptr, x, y, shift, alt, command, doc.ptr
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Handle a drag event on the active tool.
    ///
    /// - Returns: The resulting ToolAction as JSON, or nil on error.
    public func onDrag(
        x: Double, y: Double,
        shift: Bool, alt: Bool, command: Bool,
        doc: MagicDocumentState
    ) -> String? {
        guard let cstr = divi_magic_tool_on_drag(
            ptr, x, y, shift, alt, command, doc.ptr
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Handle a release event on the active tool.
    ///
    /// - Returns: The resulting ToolAction as JSON, or nil on error.
    public func onRelease(
        x: Double, y: Double,
        shift: Bool, alt: Bool, command: Bool,
        doc: MagicDocumentState
    ) -> String? {
        guard let cstr = divi_magic_tool_on_release(
            ptr, x, y, shift, alt, command, doc.ptr
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Handle a hover event on the active tool.
    ///
    /// - Returns: The cursor style as JSON, or nil if no override / no active tool.
    public func onHover(
        x: Double, y: Double,
        doc: MagicDocumentState
    ) -> String? {
        guard let cstr = divi_magic_tool_on_hover(
            ptr, x, y, doc.ptr
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - MagicDocumentHistory

/// Swift wrapper around the Rust DocumentHistory (undo/redo stack).
///
/// Tracks document actions for undo and redo. Methods that modify the
/// document take both the history and a MagicDocumentState since undo/redo
/// must apply inverse operations to the document.
///
/// ```swift
/// let history = MagicDocumentHistory()
/// let opJSON = history.execute(doc: doc, actionJSON: action)
/// if history.canUndo { let undoJSON = history.undo(doc: doc) }
/// ```
public final class MagicDocumentHistory: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new empty document history.
    public init() {
        ptr = divi_magic_history_new()!
    }

    deinit {
        divi_magic_history_free(ptr)
    }

    // MARK: - State

    /// Whether undo is available.
    public var canUndo: Bool {
        divi_magic_history_can_undo(ptr)
    }

    /// Whether redo is available.
    public var canRedo: Bool {
        divi_magic_history_can_redo(ptr)
    }

    /// The number of undo entries.
    public var undoCount: Int {
        Int(divi_magic_history_undo_count(ptr))
    }

    /// The number of redo entries.
    public var redoCount: Int {
        Int(divi_magic_history_redo_count(ptr))
    }

    /// Clear all undo/redo history.
    public func clear() {
        divi_magic_history_clear(ptr)
    }

    // MARK: - Actions

    /// Execute an action: apply it to the document and record undo history.
    ///
    /// - Parameters:
    ///   - doc: The document to apply the action to.
    ///   - actionJSON: The action to execute, serialized as JSON.
    /// - Returns: The resulting operation as JSON, or nil on error.
    public func execute(doc: MagicDocumentState, actionJSON: String) -> String? {
        guard let cstr = divi_magic_history_execute(ptr, doc.ptr, actionJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Undo the last action.
    ///
    /// - Parameter doc: The document to apply the inverse operation to.
    /// - Returns: The undo operation as JSON, or nil if nothing to undo / error.
    public func undo(doc: MagicDocumentState) -> String? {
        guard let cstr = divi_magic_history_undo(ptr, doc.ptr) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Redo the last undone action.
    ///
    /// - Parameter doc: The document to apply the redo operation to.
    /// - Returns: The redo operation as JSON, or nil if nothing to redo / error.
    public func redo(doc: MagicDocumentState) -> String? {
        guard let cstr = divi_magic_history_redo(ptr, doc.ptr) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - MagicProjection

/// Code generation from digits — design becomes code via live projection.
///
/// All methods are stateless (no opaque pointer). Digit trees are projected
/// into platform-specific code (SwiftUI, React, HTML/CSS).
///
/// ```swift
/// let ctxJSON = MagicProjection.buildContext(digitsJSON: digits, rootId: nil, reignJSON: reign)
/// let swiftFiles = MagicProjection.swiftui(contextJSON: ctxJSON)
/// ```
public enum MagicProjection {

    /// Build a ProjectionContext from digits and a Reign theme.
    ///
    /// - Parameters:
    ///   - digitsJSON: A JSON array of digits.
    ///   - rootId: Optional root digit UUID string.
    ///   - reignJSON: The Reign theme, serialized as JSON.
    /// - Returns: The ProjectionContext as JSON, or nil on error.
    public static func buildContext(
        digitsJSON: String, rootId: String?, reignJSON: String
    ) -> String? {
        guard let cstr = divi_magic_projection_build_context(
            digitsJSON, rootId, reignJSON
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Project SwiftUI code from a ProjectionContext.
    ///
    /// - Parameter contextJSON: The ProjectionContext, serialized as JSON.
    /// - Returns: A JSON array of GeneratedFile, or nil on error.
    public static func swiftui(contextJSON: String) -> String? {
        guard let cstr = divi_magic_projection_swiftui(contextJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Project React (TSX) code from a ProjectionContext.
    ///
    /// - Parameter contextJSON: The ProjectionContext, serialized as JSON.
    /// - Returns: A JSON array of GeneratedFile, or nil on error.
    public static func react(contextJSON: String) -> String? {
        guard let cstr = divi_magic_projection_react(contextJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Project HTML/CSS code from a ProjectionContext.
    ///
    /// - Parameter contextJSON: The ProjectionContext, serialized as JSON.
    /// - Returns: A JSON array of GeneratedFile, or nil on error.
    public static func html(contextJSON: String) -> String? {
        guard let cstr = divi_magic_projection_html(contextJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // NOTE: CodeProjection is imported in the Rust FFI but does not yet have
    // a `divi_magic_projection_code` extern function. Add the FFI function
    // in magic_ffi.rs first, then uncomment:
    //
    // public static func code(contextJSON: String) -> String? { ... }
}

// MARK: - MagicUtilities

/// Stateless utility functions for accessibility, slides, and type registry.
public enum MagicUtilities {

    /// Build an AccessibilitySpec from a digit, role, and label.
    ///
    /// - Parameters:
    ///   - digitJSON: The digit, serialized as JSON.
    ///   - roleJSON: The accessibility role, serialized as JSON.
    ///   - label: A human-readable accessibility label.
    /// - Returns: The AccessibilitySpec as JSON, or nil on error.
    public static func accessibilityFromDigit(
        digitJSON: String, roleJSON: String, label: String
    ) -> String? {
        guard let cstr = divi_magic_accessibility_from_digit(
            digitJSON, roleJSON, label
        ) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Slide Sequence

    /// Create a new default SlideSequenceState as JSON.
    public static func slideSequenceNew() -> String {
        let cstr = divi_magic_slide_sequence_new()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Advance to the next slide.
    ///
    /// - Parameter stateJSON: The current slide state, serialized as JSON.
    /// - Returns: The updated state as JSON, or nil on error.
    public static func slideSequenceNext(stateJSON: String) -> String? {
        guard let cstr = divi_magic_slide_sequence_next(stateJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Go to the previous slide.
    ///
    /// - Parameter stateJSON: The current slide state, serialized as JSON.
    /// - Returns: The updated state as JSON, or nil on error.
    public static func slideSequencePrevious(stateJSON: String) -> String? {
        guard let cstr = divi_magic_slide_sequence_previous(stateJSON) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Jump to a specific slide index.
    ///
    /// - Parameters:
    ///   - stateJSON: The current slide state, serialized as JSON.
    ///   - index: The slide index to jump to.
    /// - Returns: The updated state as JSON, or nil on error.
    public static func slideSequenceGoTo(stateJSON: String, index: Int) -> String? {
        guard let cstr = divi_magic_slide_sequence_go_to(stateJSON, UInt(index)) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Insert a slide at the end of the sequence.
    ///
    /// - Parameters:
    ///   - stateJSON: The current slide state, serialized as JSON.
    ///   - slideId: The UUID string of the slide to add.
    /// - Returns: The updated state as JSON, or nil on error.
    public static func slideSequenceAddSlide(stateJSON: String, slideId: String) -> String? {
        guard let cstr = divi_magic_slide_sequence_add_slide(stateJSON, slideId) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Type Registry

    /// Returns the core type registry (9 built-in digit types) as JSON.
    public static func typeRegistryCore() -> String {
        let cstr = divi_magic_type_registry_core()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a specific type definition from a registry.
    ///
    /// - Parameters:
    ///   - registryJSON: The type registry, serialized as JSON.
    ///   - digitType: The digit type string to look up.
    /// - Returns: The DigitTypeDefinition as JSON, or nil if not found.
    public static func typeRegistryGet(registryJSON: String, digitType: String) -> String? {
        guard let cstr = divi_magic_type_registry_get(registryJSON, digitType) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}
