//! Canvas interaction state — viewport, zoom, selection, snapping, and
//! coordinate transforms.
//!
//! `CanvasState` is the central hub for canvas interaction. It owns the
//! viewport transform, selection state, grid/snap config, and provides
//! coordinate conversions between screen space and canvas space.
//!
//! Ported from Swiftlight's `CanvasState`, simplified for the protocol layer.

use uuid::Uuid;

use x::geometry::{Point, Rect, Size, Transform, Vector2};

/// Canvas interaction state.
///
/// This struct manages the viewport, zoom, selection, and snapping
/// configuration for a canvas. It does NOT own the document (that is
/// `DocumentState` in Ideation) — instead it provides coordinate
/// transforms and viewport math that tools and renderers need.
///
/// # Coordinate Spaces
///
/// - **Screen space** — pixel coordinates on the display (origin at top-left
///   of the canvas view).
/// - **Canvas space** — the infinite document coordinate system. Center-origin
///   rectangles, just like X's geometry types.
///
/// The transform from canvas to screen is:
/// ```text
/// screen = (canvas - viewport_center + view_center) * zoom + pan_offset
/// ```
///
/// Or in matrix form: translate(-viewport_center) * scale(zoom) * translate(view_center + pan_offset)
#[derive(Clone, Debug)]
pub struct CanvasState {
    /// The visible portion of the canvas, in canvas coordinates.
    pub viewport: Rect,
    /// Current zoom level (1.0 = 100%). Clamped between `MIN_ZOOM` and `MAX_ZOOM`.
    zoom: f64,
    /// Accumulated pan offset in canvas coordinates.
    pub pan_offset: Vector2,
    /// Currently selected digit IDs.
    selection: Vec<Uuid>,
    /// Grid spacing in canvas units. `None` means no grid.
    pub grid_size: Option<f64>,
    /// Whether snapping to the grid is enabled.
    pub snap_to_grid: bool,
    /// Whether alignment guides are visible.
    pub show_guides: bool,
}

/// Minimum allowed zoom level.
pub const MIN_ZOOM: f64 = 0.01;
/// Maximum allowed zoom level.
pub const MAX_ZOOM: f64 = 64.0;

impl CanvasState {
    /// Creates a new canvas state with default viewport settings.
    pub fn new() -> Self {
        Self {
            viewport: Rect::new(0.0, 0.0, 1024.0, 768.0),
            zoom: 1.0,
            pan_offset: Vector2::ZERO,
            selection: Vec::new(),
            grid_size: None,
            snap_to_grid: false,
            show_guides: true,
        }
    }

    /// Creates a canvas state with a specific viewport size.
    pub fn with_viewport(width: f64, height: f64) -> Self {
        Self {
            viewport: Rect::new(0.0, 0.0, width, height),
            ..Self::new()
        }
    }

    // -----------------------------------------------------------------------
    // Coordinate transforms
    // -----------------------------------------------------------------------

    /// Converts a screen-space point to canvas-space.
    ///
    /// This is the inverse of `canvas_to_screen`.
    pub fn screen_to_canvas(&self, screen_point: Point) -> Point {
        let view_center = self.viewport.center();
        Point::new(
            (screen_point.x - view_center.x) / self.zoom + view_center.x - self.pan_offset.x,
            (screen_point.y - view_center.y) / self.zoom + view_center.y - self.pan_offset.y,
        )
    }

    /// Converts a canvas-space point to screen-space.
    pub fn canvas_to_screen(&self, canvas_point: Point) -> Point {
        let view_center = self.viewport.center();
        Point::new(
            (canvas_point.x - view_center.x + self.pan_offset.x) * self.zoom + view_center.x,
            (canvas_point.y - view_center.y + self.pan_offset.y) * self.zoom + view_center.y,
        )
    }

    /// Returns the full canvas-to-screen transform as a `Transform`.
    pub fn canvas_to_screen_transform(&self) -> Transform {
        let view_center = self.viewport.center();
        // 1. Translate by pan_offset - viewport_center
        // 2. Scale by zoom
        // 3. Translate by viewport_center
        Transform::translate(-view_center.x + self.pan_offset.x, -view_center.y + self.pan_offset.y)
            .concatenate(Transform::scale(self.zoom))
            .concatenate(Transform::translate(view_center.x, view_center.y))
    }

    // -----------------------------------------------------------------------
    // Zoom
    // -----------------------------------------------------------------------

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f64 {
        self.zoom
    }

    /// Sets the zoom level, clamping to valid range.
    ///
    /// Guards against NaN and Infinity.
    pub fn set_zoom(&mut self, level: f64) {
        if level.is_finite() {
            self.zoom = level.clamp(MIN_ZOOM, MAX_ZOOM);
        }
    }

    /// Zooms by a multiplicative factor around a center point (canvas coords).
    ///
    /// The center point stays fixed on screen after the zoom.
    pub fn zoom_by(&mut self, factor: f64, center: Point) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }

        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let actual_factor = new_zoom / self.zoom;

        // Adjust pan so that `center` stays at the same screen position
        let view_center = self.viewport.center();
        let offset_from_center = Vector2::new(
            center.x - view_center.x + self.pan_offset.x,
            center.y - view_center.y + self.pan_offset.y,
        );
        self.pan_offset -= offset_from_center * (1.0 - actual_factor);
        self.zoom = new_zoom;
    }

    /// Zooms to fit a rectangle in the viewport.
    pub fn zoom_to_fit(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }

        let scale_x = self.viewport.width / rect.width;
        let scale_y = self.viewport.height / rect.height;
        let new_zoom = scale_x.min(scale_y) * 0.9; // 10% padding

        self.set_zoom(new_zoom);
        self.pan_offset = Vector2::new(
            rect.x - self.viewport.x,
            rect.y - self.viewport.y,
        );
    }

    /// Zooms to fit all currently selected digits.
    ///
    /// Requires the caller to provide the bounding rect of the selection.
    pub fn zoom_to_selection(&mut self, selection_bounds: Rect) {
        self.zoom_to_fit(selection_bounds);
    }

    /// Zooms to fit a page of the given size at the viewport center.
    pub fn zoom_to_fit_page(&mut self, page_size: Size) {
        let page_rect = Rect::from_center_size(Point::ZERO, page_size);
        self.zoom_to_fit(page_rect);
    }

    /// Sets zoom to 100% (actual size).
    pub fn zoom_actual_size(&mut self) {
        self.zoom = 1.0;
    }

    /// Sets zoom to a specific percentage (e.g., 50.0 for 50%).
    pub fn zoom_percent(&mut self, percent: f64) {
        self.set_zoom(percent / 100.0);
    }

    // -----------------------------------------------------------------------
    // Selection
    // -----------------------------------------------------------------------

    /// Returns the current selection.
    pub fn selection(&self) -> &[Uuid] {
        &self.selection
    }

    /// Selects a single digit, replacing the current selection.
    pub fn select(&mut self, id: Uuid) {
        self.selection = vec![id];
    }

    /// Adds multiple digits to the selection.
    pub fn select_multiple(&mut self, ids: &[Uuid]) {
        for id in ids {
            if !self.selection.contains(id) {
                self.selection.push(*id);
            }
        }
    }

    /// Removes a digit from the selection.
    pub fn deselect(&mut self, id: Uuid) {
        self.selection.retain(|&s| s != id);
    }

    /// Selects all the given IDs (replaces current selection).
    pub fn select_all(&mut self, ids: Vec<Uuid>) {
        self.selection = ids;
    }

    /// Clears the selection entirely.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Returns whether a digit is currently selected.
    pub fn is_selected(&self, id: Uuid) -> bool {
        self.selection.contains(&id)
    }

    /// Returns the number of selected items.
    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    // -----------------------------------------------------------------------
    // Snapping
    // -----------------------------------------------------------------------

    /// Snaps a point to the grid if snapping is enabled and a grid is set.
    pub fn snap_point(&self, point: Point) -> Point {
        if !self.snap_to_grid {
            return point;
        }

        match self.grid_size {
            Some(size) if size > 0.0 => Point::new(
                (point.x / size).round() * size,
                (point.y / size).round() * size,
            ),
            _ => point,
        }
    }
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CanvasState {
    fn eq(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && (self.zoom - other.zoom).abs() < f64::EPSILON
            && self.pan_offset == other.pan_offset
            && self.selection == other.selection
            && self.grid_size == other.grid_size
            && self.snap_to_grid == other.snap_to_grid
            && self.show_guides == other.show_guides
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    #[test]
    fn default_state() {
        let state = CanvasState::new();
        assert!((state.zoom_level() - 1.0).abs() < EPSILON);
        assert!(state.selection().is_empty());
        assert!(!state.snap_to_grid);
        assert!(state.show_guides);
    }

    #[test]
    fn with_viewport() {
        let state = CanvasState::with_viewport(1920.0, 1080.0);
        assert!((state.viewport.width - 1920.0).abs() < EPSILON);
        assert!((state.viewport.height - 1080.0).abs() < EPSILON);
    }

    // -- Coordinate transforms --

    #[test]
    fn screen_to_canvas_identity_at_1x() {
        let state = CanvasState::new();
        let screen = Point::new(100.0, 200.0);
        let canvas = state.screen_to_canvas(screen);
        // At 1x zoom with no pan, should be identity
        assert!(
            canvas.is_approximately_equal(screen, EPSILON),
            "expected {:?}, got {:?}",
            screen,
            canvas
        );
    }

    #[test]
    fn canvas_to_screen_identity_at_1x() {
        let state = CanvasState::new();
        let canvas = Point::new(100.0, 200.0);
        let screen = state.canvas_to_screen(canvas);
        assert!(
            screen.is_approximately_equal(canvas, EPSILON),
            "expected {:?}, got {:?}",
            canvas,
            screen
        );
    }

    #[test]
    fn screen_canvas_roundtrip() {
        let mut state = CanvasState::new();
        state.set_zoom(2.0);
        state.pan_offset = Vector2::new(50.0, -30.0);

        let original = Point::new(123.0, 456.0);
        let screen = state.canvas_to_screen(original);
        let back = state.screen_to_canvas(screen);
        assert!(
            back.is_approximately_equal(original, EPSILON),
            "roundtrip failed: {:?} -> {:?} -> {:?}",
            original,
            screen,
            back
        );
    }

    #[test]
    fn screen_canvas_roundtrip_zoomed_panned() {
        let mut state = CanvasState::with_viewport(800.0, 600.0);
        state.set_zoom(3.5);
        state.pan_offset = Vector2::new(-100.0, 200.0);

        let original = Point::new(-50.0, 300.0);
        let screen = state.canvas_to_screen(original);
        let back = state.screen_to_canvas(screen);
        assert!(
            back.is_approximately_equal(original, EPSILON),
            "roundtrip failed: {:?} -> {:?} -> {:?}",
            original,
            screen,
            back
        );
    }

    // -- Zoom --

    #[test]
    fn set_zoom_clamps() {
        let mut state = CanvasState::new();
        state.set_zoom(0.001);
        assert!((state.zoom_level() - MIN_ZOOM).abs() < EPSILON);

        state.set_zoom(100.0);
        assert!((state.zoom_level() - MAX_ZOOM).abs() < EPSILON);
    }

    #[test]
    fn set_zoom_rejects_nan() {
        let mut state = CanvasState::new();
        state.set_zoom(f64::NAN);
        assert!((state.zoom_level() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn set_zoom_rejects_infinity() {
        let mut state = CanvasState::new();
        state.set_zoom(f64::INFINITY);
        assert!((state.zoom_level() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn zoom_by_factor() {
        let mut state = CanvasState::new();
        state.zoom_by(2.0, Point::ZERO);
        assert!((state.zoom_level() - 2.0).abs() < EPSILON);
    }

    #[test]
    fn zoom_by_invalid_factor_ignored() {
        let mut state = CanvasState::new();
        let zoom_before = state.zoom_level();
        state.zoom_by(f64::NAN, Point::ZERO);
        assert!((state.zoom_level() - zoom_before).abs() < EPSILON);

        state.zoom_by(-1.0, Point::ZERO);
        assert!((state.zoom_level() - zoom_before).abs() < EPSILON);
    }

    #[test]
    fn zoom_to_fit_rect() {
        let mut state = CanvasState::with_viewport(1000.0, 800.0);
        let rect = Rect::new(0.0, 0.0, 500.0, 400.0);
        state.zoom_to_fit(rect);

        // Should scale to fit with 10% padding
        let expected_zoom = (1000.0_f64 / 500.0).min(800.0 / 400.0) * 0.9;
        assert!(
            (state.zoom_level() - expected_zoom).abs() < EPSILON,
            "expected {}, got {}",
            expected_zoom,
            state.zoom_level()
        );
    }

    #[test]
    fn zoom_to_fit_empty_rect_no_op() {
        let mut state = CanvasState::new();
        let zoom_before = state.zoom_level();
        state.zoom_to_fit(Rect::ZERO);
        assert!((state.zoom_level() - zoom_before).abs() < EPSILON);
    }

    #[test]
    fn zoom_actual_size() {
        let mut state = CanvasState::new();
        state.set_zoom(3.0);
        state.zoom_actual_size();
        assert!((state.zoom_level() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn zoom_percent() {
        let mut state = CanvasState::new();
        state.zoom_percent(200.0);
        assert!((state.zoom_level() - 2.0).abs() < EPSILON);

        state.zoom_percent(50.0);
        assert!((state.zoom_level() - 0.5).abs() < EPSILON);
    }

    #[test]
    fn zoom_to_fit_page() {
        let mut state = CanvasState::with_viewport(1000.0, 800.0);
        state.zoom_to_fit_page(Size::new(595.0, 842.0));
        // Should fit A4 within the viewport
        assert!(state.zoom_level() > 0.0);
        assert!(state.zoom_level() < 2.0);
    }

    // -- Selection --

    #[test]
    fn select_single() {
        let mut state = CanvasState::new();
        let id = Uuid::new_v4();
        state.select(id);
        assert_eq!(state.selection(), &[id]);
        assert!(state.is_selected(id));
        assert_eq!(state.selection_count(), 1);
    }

    #[test]
    fn select_replaces_previous() {
        let mut state = CanvasState::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        state.select(a);
        state.select(b);
        assert_eq!(state.selection(), &[b]);
        assert!(!state.is_selected(a));
    }

    #[test]
    fn select_multiple() {
        let mut state = CanvasState::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        state.select(a);
        state.select_multiple(&[b, c]);
        assert_eq!(state.selection_count(), 3);
        assert!(state.is_selected(a));
        assert!(state.is_selected(b));
        assert!(state.is_selected(c));
    }

    #[test]
    fn select_multiple_deduplicates() {
        let mut state = CanvasState::new();
        let a = Uuid::new_v4();
        state.select(a);
        state.select_multiple(&[a, a]);
        assert_eq!(state.selection_count(), 1);
    }

    #[test]
    fn deselect() {
        let mut state = CanvasState::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        state.select_all(vec![a, b]);
        state.deselect(a);
        assert!(!state.is_selected(a));
        assert!(state.is_selected(b));
    }

    #[test]
    fn clear_selection() {
        let mut state = CanvasState::new();
        state.select_all(vec![Uuid::new_v4(), Uuid::new_v4()]);
        state.clear_selection();
        assert!(state.selection().is_empty());
    }

    #[test]
    fn select_all_replaces() {
        let mut state = CanvasState::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        state.select(a);
        state.select_all(vec![b]);
        assert!(!state.is_selected(a));
        assert!(state.is_selected(b));
    }

    // -- Snapping --

    #[test]
    fn snap_disabled_returns_original() {
        let state = CanvasState::new();
        let p = Point::new(17.3, 23.7);
        let snapped = state.snap_point(p);
        assert!(snapped.is_approximately_equal(p, EPSILON));
    }

    #[test]
    fn snap_to_grid_rounds() {
        let mut state = CanvasState::new();
        state.snap_to_grid = true;
        state.grid_size = Some(10.0);

        let p = Point::new(17.3, 23.7);
        let snapped = state.snap_point(p);
        assert!((snapped.x - 20.0).abs() < EPSILON);
        assert!((snapped.y - 20.0).abs() < EPSILON);
    }

    #[test]
    fn snap_no_grid_size_returns_original() {
        let mut state = CanvasState::new();
        state.snap_to_grid = true;
        // grid_size is None

        let p = Point::new(17.3, 23.7);
        let snapped = state.snap_point(p);
        assert!(snapped.is_approximately_equal(p, EPSILON));
    }

    #[test]
    fn snap_zero_grid_size_returns_original() {
        let mut state = CanvasState::new();
        state.snap_to_grid = true;
        state.grid_size = Some(0.0);

        let p = Point::new(17.3, 23.7);
        let snapped = state.snap_point(p);
        assert!(snapped.is_approximately_equal(p, EPSILON));
    }

    // -- Equality --

    #[test]
    fn partial_eq() {
        let a = CanvasState::new();
        let b = CanvasState::new();
        assert_eq!(a, b);
    }
}
