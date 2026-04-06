use std::collections::HashMap;

use crate::error::RegaliaError;
use crate::insignia::{Border, BorderInsets, SanctumID};
use crate::sanctum::Sanctum;

use super::appointment::Frame;
use super::{Appointment, Clansman, Domain};

/// Max recursion depth for nested subsanctums.
pub const MAX_DEPTH: usize = 8;

/// Accumulator for resolved layout data during Arbiter traversal.
struct ResolveAccum {
    appointments: Vec<Appointment>,
    sanctum_clips: HashMap<String, Frame>,
    sanctum_bounds: HashMap<String, Frame>,
}

/// The layout solver. Takes sanctums + clansmen, produces a fully-resolved Domain.
pub struct Arbiter;

impl Arbiter {
    /// Resolve a complete layout.
    pub fn resolve(
        bounds: Frame,
        sanctums: &[Sanctum],
        vassals: &HashMap<SanctumID, Vec<&dyn Clansman>>,
        sanctum_insets: &HashMap<SanctumID, BorderInsets>,
        formation_resolver: Option<crate::formation::FormationResolver<'_>>,
    ) -> Result<Domain, RegaliaError> {
        let mut accum = ResolveAccum {
            appointments: Vec::new(),
            sanctum_clips: HashMap::new(),
            sanctum_bounds: HashMap::new(),
        };

        let allocated = Self::allocate_sanctums(bounds, sanctums);

        for (sanctum, sbounds) in sanctums.iter().zip(allocated.iter()) {
            Self::resolve_sanctum(
                sanctum,
                *sbounds,
                vassals,
                sanctum_insets,
                formation_resolver,
                &mut accum,
                0,
            )?;
        }

        Ok(Domain::new(
            accum.appointments,
            accum.sanctum_clips,
            accum.sanctum_bounds,
            bounds,
        ))
    }

    /// Allocate bounds for sanctums based on their border attachment and fixedExtent.
    fn allocate_sanctums(bounds: Frame, sanctums: &[Sanctum]) -> Vec<Frame> {
        let (mut x, mut y, mut w, mut h) = bounds;
        let mut results = vec![(0.0, 0.0, 0.0, 0.0); sanctums.len()];

        // First pass: allocate border-attached sanctums (carve from remaining space)
        for (i, sanctum) in sanctums.iter().enumerate() {
            if let Some(border) = &sanctum.border {
                match border {
                    Border::Top => {
                        let extent = sanctum.fixed_extent.unwrap_or(44.0);
                        results[i] = (x, y, w, extent);
                        y += extent;
                        h -= extent;
                    }
                    Border::Bottom => {
                        let extent = sanctum.fixed_extent.unwrap_or(44.0);
                        results[i] = (x, y + h - extent, w, extent);
                        h -= extent;
                    }
                    Border::Leading => {
                        let extent = sanctum.fixed_extent.unwrap_or(240.0);
                        results[i] = (x, y, extent, h);
                        x += extent;
                        w -= extent;
                    }
                    Border::Trailing => {
                        let extent = sanctum.fixed_extent.unwrap_or(240.0);
                        results[i] = (x + w - extent, y, extent, h);
                        w -= extent;
                    }
                }
            }
        }

        // Second pass: free-form sanctums get remaining space
        for (i, sanctum) in sanctums.iter().enumerate() {
            if sanctum.border.is_none() {
                results[i] = (x, y, w, h);
            }
        }

        results
    }

    fn resolve_sanctum(
        sanctum: &Sanctum,
        sbounds: Frame,
        vassals: &HashMap<SanctumID, Vec<&dyn Clansman>>,
        sanctum_insets: &HashMap<SanctumID, BorderInsets>,
        formation_resolver: Option<crate::formation::FormationResolver<'_>>,
        accum: &mut ResolveAccum,
        depth: usize,
    ) -> Result<(), RegaliaError> {
        if depth > MAX_DEPTH {
            return Err(RegaliaError::NestingTooDeep {
                id: sanctum.id.as_str().to_string(),
                max: MAX_DEPTH,
            });
        }

        accum
            .sanctum_bounds
            .insert(sanctum.id.as_str().to_string(), sbounds);

        if sanctum.clips {
            accum
                .sanctum_clips
                .insert(sanctum.id.as_str().to_string(), sbounds);
        }

        let insets = sanctum_insets
            .get(&sanctum.id)
            .copied()
            .unwrap_or_default();
        let (cx, cy, cw, ch) = insets.inset(sbounds.0, sbounds.1, sbounds.2, sbounds.3);

        if !sanctum.subsanctums.is_empty() {
            let sub_allocated =
                Self::allocate_sanctums((cx, cy, cw, ch), &sanctum.subsanctums);
            for (sub, sub_bounds) in sanctum.subsanctums.iter().zip(sub_allocated.iter()) {
                Self::resolve_sanctum(
                    sub,
                    *sub_bounds,
                    vassals,
                    sanctum_insets,
                    formation_resolver,
                    accum,
                    depth + 1,
                )?;
            }
        }

        if let Some(children) = vassals.get(&sanctum.id) {
            let formation = sanctum.formation_kind.make_formation(formation_resolver);
            let decrees = formation.place_children(cx, cy, cw, ch, children);

            for (decree, child) in decrees.iter().zip(children.iter()) {
                let appointment = Appointment::new(
                    child.id(),
                    (decree.x, decree.y, decree.width, decree.height),
                    sanctum.id.clone(),
                    sanctum.z_layer,
                    decree.z_index,
                );
                accum.appointments.push(appointment);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MockClansman;
    use crate::formation::FormationKind;

    fn make_sanctums() -> Vec<Sanctum> {
        vec![
            Sanctum::sidebar(None, None),
            Sanctum::toolbar(None, None),
            Sanctum::content(None),
        ]
    }

    #[test]
    fn basic_layout() {
        let sanctums = make_sanctums();
        let c1 = MockClansman::named("widget-a", Some((100.0, 50.0)));
        let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
            SanctumID::content(),
            vec![&c1 as &dyn Clansman],
        )]);

        let domain = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &vassals,
            &HashMap::new(),
            None,
        )
        .unwrap();

        assert!(!domain.appointments.is_empty());
        assert!(domain.sanctum_bounds.contains_key("sidebar"));
        assert!(domain.sanctum_bounds.contains_key("toolbar"));
        assert!(domain.sanctum_bounds.contains_key("content"));
    }

    #[test]
    fn sidebar_carves_space() {
        let sanctums = vec![Sanctum::sidebar(Some(200.0), None)];
        let domain = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &HashMap::new(),
            &HashMap::new(),
            None,
        )
        .unwrap();

        let sb = domain.sanctum_bounds_for(&SanctumID::sidebar()).unwrap();
        assert_eq!(sb.2, 200.0);
    }

    #[test]
    fn toolbar_carves_space() {
        let sanctums = vec![Sanctum::toolbar(Some(60.0), None)];
        let domain = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &HashMap::new(),
            &HashMap::new(),
            None,
        )
        .unwrap();

        let tb = domain.sanctum_bounds_for(&SanctumID::toolbar()).unwrap();
        assert_eq!(tb.3, 60.0);
    }

    #[test]
    fn content_gets_remaining() {
        let sanctums = vec![
            Sanctum::sidebar(Some(200.0), None),
            Sanctum::toolbar(Some(44.0), None),
            Sanctum::content(None),
        ];
        let domain = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &HashMap::new(),
            &HashMap::new(),
            None,
        )
        .unwrap();

        let content = domain.sanctum_bounds_for(&SanctumID::content()).unwrap();
        assert_eq!(content.0, 200.0);
        assert_eq!(content.1, 44.0);
        assert_eq!(content.2, 600.0);
        assert_eq!(content.3, 556.0);
    }

    #[test]
    fn formation_places_children() {
        let sanctums = vec![Sanctum {
            id: SanctumID::content(),
            border: None,
            fixed_extent: None,
            seat: crate::insignia::Seat::Center,
            z_layer: 0,
            clips: true,
            formation_kind: FormationKind::Column {
                spacing: 10.0,
                alignment: crate::formation::ColumnAlignment::Leading,
                justification: crate::formation::ColumnJustification::Top,
            },
            subsanctums: vec![],
        }];

        let c1 = MockClansman::named("a", Some((100.0, 30.0)));
        let c2 = MockClansman::named("b", Some((100.0, 30.0)));
        let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
            SanctumID::content(),
            vec![&c1 as &dyn Clansman, &c2 as &dyn Clansman],
        )]);

        let domain = Arbiter::resolve(
            (0.0, 0.0, 400.0, 300.0),
            &sanctums,
            &vassals,
            &HashMap::new(),
            None,
        )
        .unwrap();

        assert_eq!(domain.appointments.len(), 2);
        assert!(domain.appointments[1].y > domain.appointments[0].y);
    }

    #[test]
    fn nesting_depth_limit() {
        fn deeply_nested(depth: usize) -> Sanctum {
            if depth == 0 {
                Sanctum::content(None)
            } else {
                Sanctum {
                    id: SanctumID::new(format!("level-{depth}")),
                    border: None,
                    fixed_extent: None,
                    seat: crate::insignia::Seat::Center,
                    z_layer: 0,
                    clips: false,
                    formation_kind: FormationKind::OpenCourt,
                    subsanctums: vec![deeply_nested(depth - 1)],
                }
            }
        }

        let sanctums = vec![deeply_nested(8)];
        let result = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &HashMap::new(),
            &HashMap::new(),
            None,
        );
        assert!(result.is_ok());

        let sanctums = vec![deeply_nested(10)];
        let result = Arbiter::resolve(
            (0.0, 0.0, 800.0, 600.0),
            &sanctums,
            &HashMap::new(),
            &HashMap::new(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn content_insets_applied() {
        let sanctums = vec![Sanctum::content(None)];
        let c1 = MockClansman::named("widget", Some((50.0, 50.0)));
        let vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::from([(
            SanctumID::content(),
            vec![&c1 as &dyn Clansman],
        )]);
        let insets = HashMap::from([(SanctumID::content(), BorderInsets::uniform(20.0))]);

        let domain = Arbiter::resolve(
            (0.0, 0.0, 200.0, 200.0),
            &sanctums,
            &vassals,
            &insets,
            None,
        )
        .unwrap();

        assert!(domain.appointments[0].x >= 20.0);
        assert!(domain.appointments[0].y >= 20.0);
    }
}
