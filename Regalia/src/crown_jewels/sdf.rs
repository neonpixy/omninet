use crate::crown_jewels::shape::CornerRadii;

/// Signed distance to a rounded rectangle. Negative inside, positive outside,
/// zero on the boundary.
///
/// Based on Inigo Quilez's rounded box SDF. Selects corner radius per quadrant.
pub fn sdf_rounded_rect(
    px: f64,
    py: f64,
    half_w: f64,
    half_h: f64,
    radii: &CornerRadii,
    _smoothing: f64,
) -> f64 {
    // Select corner radius based on quadrant.
    let r = if px >= 0.0 {
        if py >= 0.0 {
            radii.bottom_right
        } else {
            radii.top_right
        }
    } else if py >= 0.0 {
        radii.bottom_left
    } else {
        radii.top_left
    };

    let r = r.min(half_w).min(half_h);
    let qx = px.abs() - half_w + r;
    let qy = py.abs() - half_h + r;

    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    let inside = qx.max(qy).min(0.0);

    outside + inside - r
}

/// Signed distance to an ellipse. Simple algebraic approach:
/// distance to the boundary using normalized coordinates.
pub fn sdf_ellipse(px: f64, py: f64, rx: f64, ry: f64) -> f64 {
    if rx < 1e-10 || ry < 1e-10 {
        return (px * px + py * py).sqrt();
    }

    // Normalized distance: 1.0 on boundary, <1 inside, >1 outside.
    let nx = px / rx;
    let ny = py / ry;
    let norm = (nx * nx + ny * ny).sqrt();

    if norm < 1e-10 {
        // At center, distance = min radius.
        return -rx.min(ry);
    }

    // Closest point on ellipse along the radial direction.
    let closest_x = rx * nx / norm;
    let closest_y = ry * ny / norm;
    let dist = ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt();

    if norm < 1.0 {
        -dist
    } else {
        dist
    }
}

/// Signed distance to a regular polygon with N sides and optional corner radius.
pub fn sdf_polygon(px: f64, py: f64, radius: f64, sides: u32, corner_radius: f64) -> f64 {
    if sides < 3 {
        return (px * px + py * py).sqrt() - radius;
    }

    let n = sides as f64;
    let an = std::f64::consts::PI / n;
    let he = an.cos();

    // Fold angle into first sector.
    let angle = py.atan2(px);
    let sector = ((angle + an) / (2.0 * an)).floor();
    let local_angle = angle - sector * 2.0 * an;

    let cos_a = local_angle.cos();
    let sin_a = local_angle.sin();

    let dist_to_edge = cos_a * (px * px + py * py).sqrt() - radius * he;

    if corner_radius > 0.0 {
        let p_len = (px * px + py * py).sqrt();
        let proj_x = cos_a * p_len;
        let proj_y = sin_a.abs() * p_len;

        let half_edge = radius * an.sin();
        let cy = proj_y.clamp(0.0, half_edge - corner_radius);
        let cx = radius * he - corner_radius;

        let dx = proj_x - cx;
        let dy = proj_y - cy;
        if proj_x > cx {
            (dx * dx + dy * dy).sqrt() - corner_radius
        } else {
            dist_to_edge
        }
    } else {
        dist_to_edge
    }
}

/// Signed distance to a star shape with N points.
pub fn sdf_star(
    px: f64,
    py: f64,
    outer: f64,
    inner: f64,
    points: u32,
    _corner_radius: f64,
    _inner_corner_radius: f64,
) -> f64 {
    if points < 2 {
        return (px * px + py * py).sqrt() - outer;
    }

    let n = points as f64;
    let half_sector = std::f64::consts::PI / n;

    let p_len = (px * px + py * py).sqrt();
    if p_len < 1e-10 {
        // At center — distance is negative (inside).
        return -inner;
    }

    let angle = py.atan2(px);
    // Fold into one sector (half of a full sector between outer and inner vertices).
    let sector_angle = angle.rem_euclid(2.0 * half_sector);
    let folded_angle = if sector_angle > half_sector {
        2.0 * half_sector - sector_angle
    } else {
        sector_angle
    };

    let lx = p_len * folded_angle.cos();
    let ly = p_len * folded_angle.sin();

    // Edge from outer vertex (on x-axis) to inner vertex.
    let ax = outer;
    let ay = 0.0;
    let bx = inner * half_sector.cos();
    let by = inner * half_sector.sin();

    let ex = bx - ax;
    let ey = by - ay;
    let dx = lx - ax;
    let dy = ly - ay;

    let edge_len_sq = ex * ex + ey * ey;
    let t = ((dx * ex + dy * ey) / edge_len_sq).clamp(0.0, 1.0);

    let closest_x = ax + t * ex;
    let closest_y = ay + t * ey;
    let dist = ((lx - closest_x).powi(2) + (ly - closest_y).powi(2)).sqrt();

    // Sign: cross product determines inside/outside.
    let cross = ex * dy - ey * dx;
    if cross < 0.0 {
        -dist
    } else {
        dist
    }
}

/// Polynomial smooth minimum. Blends two SDF values smoothly.
/// k controls blend radius (0 = hard min, higher = smoother).
pub fn smooth_min(a: f64, b: f64, k: f64) -> f64 {
    if k < 1e-10 {
        return a.min(b);
    }
    let h = ((k - (a - b).abs()) / k).clamp(0.0, 1.0);
    a.min(b) - h * h * k * 0.25
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_center_is_negative() {
        let d = sdf_rounded_rect(0.0, 0.0, 50.0, 30.0, &CornerRadii::ZERO, 0.0);
        assert!(d < 0.0);
    }

    #[test]
    fn rounded_rect_outside_is_positive() {
        let d = sdf_rounded_rect(100.0, 100.0, 50.0, 30.0, &CornerRadii::ZERO, 0.0);
        assert!(d > 0.0);
    }

    #[test]
    fn rounded_rect_on_edge_near_zero() {
        // Point on the right edge, no corner radius.
        let d = sdf_rounded_rect(50.0, 0.0, 50.0, 30.0, &CornerRadii::ZERO, 0.0);
        assert!(d.abs() < 1e-6);
    }

    #[test]
    fn ellipse_center_negative() {
        let d = sdf_ellipse(0.0, 0.0, 40.0, 20.0);
        assert!(d < 0.0);
    }

    #[test]
    fn ellipse_outside_positive() {
        let d = sdf_ellipse(100.0, 0.0, 40.0, 20.0);
        assert!(d > 0.0);
    }

    #[test]
    fn polygon_center_negative() {
        let d = sdf_polygon(0.0, 0.0, 50.0, 6, 0.0);
        assert!(d < 0.0);
    }

    #[test]
    fn polygon_outside_positive() {
        let d = sdf_polygon(100.0, 0.0, 50.0, 6, 0.0);
        assert!(d > 0.0);
    }

    #[test]
    fn star_center_negative() {
        let d = sdf_star(0.0, 0.0, 50.0, 25.0, 5, 0.0, 0.0);
        assert!(d < 0.0);
    }

    #[test]
    fn smooth_min_without_smoothing() {
        let result = smooth_min(3.0, 5.0, 0.0);
        assert!((result - 3.0).abs() < 1e-10);
    }

    #[test]
    fn smooth_min_with_smoothing() {
        let result = smooth_min(1.0, 1.0, 2.0);
        // When a == b, smooth_min should return slightly less than a.
        assert!(result < 1.0);
        assert!(result > 0.0);
    }
}
