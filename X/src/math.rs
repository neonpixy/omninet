//! Pure math functions for interpolation, clamping, angles, and bezier curves.
//!
//! Ported from Swiftlight's MathUtils.swift to idiomatic Rust.

use std::f64::consts::PI;

/// Two pi (full circle in radians).
pub const TWO_PI: f64 = PI * 2.0;

/// Half pi (quarter circle in radians).
pub const HALF_PI: f64 = PI / 2.0;

/// Degrees to radians conversion factor.
pub const DEG_TO_RAD: f64 = PI / 180.0;

/// Radians to degrees conversion factor.
pub const RAD_TO_DEG: f64 = 180.0 / PI;

// ---------------------------------------------------------------------------
// Interpolation
// ---------------------------------------------------------------------------

/// Linear interpolation between two values.
///
/// Returns `a` when `t == 0`, `b` when `t == 1`.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Inverse linear interpolation — finds `t` given a value between `a` and `b`.
///
/// Returns `0.0` when `a == b` to avoid division by zero.
#[inline]
pub fn inverse_lerp(a: f64, b: f64, value: f64) -> f64 {
    let denom = b - a;
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    (value - a) / denom
}

/// Remaps a value from one range to another.
#[inline]
pub fn remap(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let t = inverse_lerp(in_min, in_max, value);
    lerp(out_min, out_max, t)
}

// ---------------------------------------------------------------------------
// Clamping
// ---------------------------------------------------------------------------

/// Clamps a value to the 0.0..=1.0 range.
#[inline]
pub fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Smooth interpolation
// ---------------------------------------------------------------------------

/// Hermite smoothstep (3-argument, clamped).
///
/// Returns 0 when `x <= edge0`, 1 when `x >= edge1`, and a smooth Hermite
/// interpolation in between.
#[inline]
pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = clamp01(inverse_lerp(edge0, edge1, x));
    t * t * (3.0 - 2.0 * t)
}

/// Perlin's improved smoothstep (6t^5 - 15t^4 + 10t^3).
#[inline]
pub fn smootherstep(t: f64) -> f64 {
    let t = clamp01(t);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

// ---------------------------------------------------------------------------
// Stepping
// ---------------------------------------------------------------------------

/// Step function: returns 0.0 if `value < edge`, else 1.0.
#[inline]
pub fn step(edge: f64, value: f64) -> f64 {
    if value < edge {
        0.0
    } else {
        1.0
    }
}

/// Moves `current` toward `target` by at most `max_delta`.
#[inline]
pub fn move_toward(current: f64, target: f64, max_delta: f64) -> f64 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

// ---------------------------------------------------------------------------
// Wrapping
// ---------------------------------------------------------------------------

/// Wraps a value into the `min..max` range (handles negatives correctly).
#[inline]
pub fn wrap(value: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    if range.abs() < f64::EPSILON {
        return min;
    }
    let mut result = (value - min) % range;
    if result < 0.0 {
        result += range;
    }
    result + min
}

/// Ping-pong: bounces a value back and forth between 0 and `length`.
#[inline]
pub fn ping_pong(value: f64, length: f64) -> f64 {
    if length.abs() < f64::EPSILON {
        return 0.0;
    }
    let t = wrap(value, 0.0, length * 2.0);
    length - (t - length).abs()
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// Returns whether two values are approximately equal within `tolerance`.
#[inline]
pub fn is_approximately(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() < tolerance
}

/// Snaps `value` to the nearest integer if within `tolerance`.
#[inline]
pub fn snap_to_integer(value: f64, tolerance: f64) -> f64 {
    let rounded = value.round();
    if (value - rounded).abs() < tolerance {
        rounded
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Angle utilities
// ---------------------------------------------------------------------------

/// Normalizes an angle to the `0..2π` range.
#[inline]
pub fn normalize_angle(radians: f64) -> f64 {
    let mut result = radians % TWO_PI;
    if result < 0.0 {
        result += TWO_PI;
    }
    result
}

/// Normalizes an angle to the `-π..π` range.
#[inline]
pub fn normalize_angle_signed(radians: f64) -> f64 {
    let mut result = normalize_angle(radians);
    if result > PI {
        result -= TWO_PI;
    }
    result
}

/// Returns the shortest angular distance between two angles (radians).
#[inline]
pub fn angle_difference(a: f64, b: f64) -> f64 {
    normalize_angle_signed(b - a)
}

/// Linearly interpolates between two angles, taking the shortest path.
#[inline]
pub fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    let diff = angle_difference(a, b);
    a + diff * t
}

// ---------------------------------------------------------------------------
// Bezier
// ---------------------------------------------------------------------------

/// Evaluates a quadratic Bezier curve at `t`.
#[inline]
pub fn quadratic_bezier(p0: f64, p1: f64, p2: f64, t: f64) -> f64 {
    let one_minus_t = 1.0 - t;
    one_minus_t * one_minus_t * p0 + 2.0 * one_minus_t * t * p1 + t * t * p2
}

/// Evaluates a cubic Bezier curve at `t`.
#[inline]
pub fn cubic_bezier(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let one_minus_t = 1.0 - t;
    let one_minus_t2 = one_minus_t * one_minus_t;
    let one_minus_t3 = one_minus_t2 * one_minus_t;
    let t2 = t * t;
    let t3 = t2 * t;
    one_minus_t3 * p0 + 3.0 * one_minus_t2 * t * p1 + 3.0 * one_minus_t * t2 * p2 + t3 * p3
}

/// Subdivides a cubic Bezier at `t` via De Casteljau's algorithm.
///
/// Returns two cubic Bezier curves `(left, right)` as tuples of 4 control points each.
#[inline]
#[allow(clippy::type_complexity)]
pub fn cubic_bezier_subdivide(
    p0: f64,
    p1: f64,
    p2: f64,
    p3: f64,
    t: f64,
) -> ((f64, f64, f64, f64), (f64, f64, f64, f64)) {
    let p01 = lerp(p0, p1, t);
    let p12 = lerp(p1, p2, t);
    let p23 = lerp(p2, p3, t);
    let p012 = lerp(p01, p12, t);
    let p123 = lerp(p12, p23, t);
    let p0123 = lerp(p012, p123, t);

    ((p0, p01, p012, p0123), (p0123, p123, p23, p3))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    // -- Interpolation --

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < EPSILON);
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < EPSILON);
        assert!((lerp(-5.0, 5.0, 0.5) - 0.0).abs() < EPSILON);
        // Extrapolation
        assert!((lerp(0.0, 10.0, 2.0) - 20.0).abs() < EPSILON);
        assert!((lerp(0.0, 10.0, -1.0) - -10.0).abs() < EPSILON);
    }

    #[test]
    fn test_inverse_lerp() {
        assert!((inverse_lerp(0.0, 10.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((inverse_lerp(0.0, 10.0, 10.0) - 1.0).abs() < EPSILON);
        assert!((inverse_lerp(0.0, 10.0, 5.0) - 0.5).abs() < EPSILON);
        // Degenerate case
        assert!((inverse_lerp(5.0, 5.0, 5.0) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_remap() {
        assert!((remap(5.0, 0.0, 10.0, 0.0, 100.0) - 50.0).abs() < EPSILON);
        assert!((remap(0.0, 0.0, 10.0, 100.0, 200.0) - 100.0).abs() < EPSILON);
        assert!((remap(10.0, 0.0, 10.0, 100.0, 200.0) - 200.0).abs() < EPSILON);
    }

    // -- Clamping --

    #[test]
    fn test_clamp01() {
        assert!((clamp01(0.5) - 0.5).abs() < EPSILON);
        assert!((clamp01(-1.0) - 0.0).abs() < EPSILON);
        assert!((clamp01(2.0) - 1.0).abs() < EPSILON);
        assert!((clamp01(0.0) - 0.0).abs() < EPSILON);
        assert!((clamp01(1.0) - 1.0).abs() < EPSILON);
    }

    // -- Smoothstep --

    #[test]
    fn test_smoothstep() {
        assert!((smoothstep(0.0, 1.0, -0.5) - 0.0).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 1.5) - 1.0).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 1.0) - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_smootherstep() {
        assert!((smootherstep(0.0) - 0.0).abs() < EPSILON);
        assert!((smootherstep(1.0) - 1.0).abs() < EPSILON);
        assert!((smootherstep(0.5) - 0.5).abs() < EPSILON);
    }

    // -- Step --

    #[test]
    fn test_step() {
        assert!((step(0.5, 0.3) - 0.0).abs() < EPSILON);
        assert!((step(0.5, 0.5) - 1.0).abs() < EPSILON);
        assert!((step(0.5, 0.7) - 1.0).abs() < EPSILON);
    }

    // -- Move toward --

    #[test]
    fn test_move_toward() {
        assert!((move_toward(0.0, 10.0, 3.0) - 3.0).abs() < EPSILON);
        assert!((move_toward(0.0, 10.0, 20.0) - 10.0).abs() < EPSILON);
        assert!((move_toward(5.0, 5.0, 1.0) - 5.0).abs() < EPSILON);
        assert!((move_toward(10.0, 0.0, 3.0) - 7.0).abs() < EPSILON);
    }

    // -- Wrap --

    #[test]
    fn test_wrap() {
        assert!((wrap(5.0, 0.0, 10.0) - 5.0).abs() < EPSILON);
        assert!((wrap(12.0, 0.0, 10.0) - 2.0).abs() < EPSILON);
        assert!((wrap(-3.0, 0.0, 10.0) - 7.0).abs() < EPSILON);
        assert!((wrap(0.0, 0.0, 10.0) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_ping_pong() {
        assert!((ping_pong(0.0, 5.0) - 0.0).abs() < EPSILON);
        assert!((ping_pong(5.0, 5.0) - 5.0).abs() < EPSILON);
        assert!((ping_pong(7.0, 5.0) - 3.0).abs() < EPSILON);
        assert!((ping_pong(10.0, 5.0) - 0.0).abs() < EPSILON);
        assert!((ping_pong(0.0, 0.0) - 0.0).abs() < EPSILON);
    }

    // -- Comparison --

    #[test]
    fn test_is_approximately() {
        assert!(is_approximately(1.0, 1.00005, 0.0001));
        assert!(!is_approximately(1.0, 1.001, 0.0001));
    }

    #[test]
    fn test_snap_to_integer() {
        assert!((snap_to_integer(2.99999, 0.0001) - 3.0).abs() < EPSILON);
        assert!((snap_to_integer(2.5, 0.0001) - 2.5).abs() < EPSILON);
        assert!((snap_to_integer(3.00001, 0.0001) - 3.0).abs() < EPSILON);
    }

    // -- Angles --

    #[test]
    fn test_normalize_angle() {
        let a = normalize_angle(0.0);
        assert!(a.abs() < EPSILON);

        let a = normalize_angle(TWO_PI + 0.5);
        assert!((a - 0.5).abs() < EPSILON);

        let a = normalize_angle(-0.5);
        assert!((a - (TWO_PI - 0.5)).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_angle_signed() {
        let a = normalize_angle_signed(0.0);
        assert!(a.abs() < EPSILON);

        let a = normalize_angle_signed(PI + 0.1);
        assert!((a - (-PI + 0.1)).abs() < 1e-9);

        let a = normalize_angle_signed(-PI + 0.1);
        assert!((a - (-PI + 0.1)).abs() < 1e-9);
    }

    #[test]
    fn test_angle_difference() {
        let d = angle_difference(0.0, PI / 2.0);
        assert!((d - PI / 2.0).abs() < EPSILON);

        // Shortest path wraps around
        let d = angle_difference(0.1, TWO_PI - 0.1);
        assert!((d - (-0.2)).abs() < 1e-9);
    }

    #[test]
    fn test_lerp_angle() {
        let a = lerp_angle(0.0, PI, 0.5);
        assert!((a - PI / 2.0).abs() < 1e-9);

        // Wrapping
        let a = lerp_angle(0.1, TWO_PI - 0.1, 0.5);
        assert!(a.abs() < 1e-9 || (a - TWO_PI).abs() < 1e-9);
    }

    // -- Bezier --

    #[test]
    fn test_quadratic_bezier() {
        assert!((quadratic_bezier(0.0, 5.0, 10.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((quadratic_bezier(0.0, 5.0, 10.0, 1.0) - 10.0).abs() < EPSILON);
        assert!((quadratic_bezier(0.0, 5.0, 10.0, 0.5) - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_cubic_bezier() {
        assert!((cubic_bezier(0.0, 0.0, 10.0, 10.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((cubic_bezier(0.0, 0.0, 10.0, 10.0, 1.0) - 10.0).abs() < EPSILON);
        // Linear control points: should be 5 at midpoint
        assert!((cubic_bezier(0.0, 0.0, 10.0, 10.0, 0.5) - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_cubic_bezier_subdivide() {
        let (left, right) = cubic_bezier_subdivide(0.0, 1.0, 2.0, 3.0, 0.5);
        // For a linear curve (0,1,2,3), subdivision at 0.5 should give:
        // left  = (0.0, 0.5, 1.0, 1.5)
        // right = (1.5, 2.0, 2.5, 3.0)
        assert!((left.0 - 0.0).abs() < EPSILON);
        assert!((left.1 - 0.5).abs() < EPSILON);
        assert!((left.2 - 1.0).abs() < EPSILON);
        assert!((left.3 - 1.5).abs() < EPSILON);
        assert!((right.0 - 1.5).abs() < EPSILON);
        assert!((right.1 - 2.0).abs() < EPSILON);
        assert!((right.2 - 2.5).abs() < EPSILON);
        assert!((right.3 - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_cubic_bezier_subdivide_evaluates_same() {
        // The point at t=0.5 on original curve should equal left.3 and right.0
        let val = cubic_bezier(2.0, 5.0, 8.0, 3.0, 0.5);
        let (left, right) = cubic_bezier_subdivide(2.0, 5.0, 8.0, 3.0, 0.5);
        assert!((left.3 - val).abs() < EPSILON);
        assert!((right.0 - val).abs() < EPSILON);
    }
}
