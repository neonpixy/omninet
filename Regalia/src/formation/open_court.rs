use super::Formation;
use crate::domain::Clansman;
use crate::insignia::Decree;

/// Free-form positioning: children specify their own position via intrinsic size.
/// Children without an intrinsic size get placed at the origin with min size.
pub struct OpenCourt;

impl Formation for OpenCourt {
    fn place_children(
        &self,
        bounds_x: f64,
        bounds_y: f64,
        _bounds_width: f64,
        _bounds_height: f64,
        children: &[&dyn Clansman],
    ) -> Vec<Decree> {
        children
            .iter()
            .map(|child| {
                let intrinsic = child.intrinsic_size();
                let (min_w, min_h) = child.min_size();

                let w = intrinsic.map(|(iw, _)| iw).unwrap_or(min_w);
                let h = intrinsic.map(|(_, ih)| ih).unwrap_or(min_h);

                Decree::new(bounds_x, bounds_y, w, h)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MockClansman;

    #[test]
    fn places_at_origin() {
        let oc = OpenCourt;
        let child = MockClansman::new(Some((50.0, 30.0)));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = oc.place_children(10.0, 20.0, 200.0, 100.0, &children);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].x, 10.0);
        assert_eq!(result[0].y, 20.0);
        assert_eq!(result[0].width, 50.0);
        assert_eq!(result[0].height, 30.0);
    }

    #[test]
    fn no_intrinsic_uses_min() {
        let oc = OpenCourt;
        let child = MockClansman::with_min(None, (60.0, 40.0));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = oc.place_children(0.0, 0.0, 200.0, 100.0, &children);
        assert_eq!(result[0].width, 60.0);
        assert_eq!(result[0].height, 40.0);
    }
}
