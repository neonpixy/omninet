use std::collections::HashMap;
use uuid::Uuid;

use serde::{Deserialize, Serialize};

use ideas::Digit;
use regalia::{Appointment, FormationKind, Reign, Sanctum};

/// Pre-computed indices for efficient projection. Built in one O(n) pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionContext {
    /// All digits indexed by ID.
    pub digit_index: HashMap<Uuid, Digit>,
    /// Parent → children mapping (ordered).
    pub children_index: HashMap<Uuid, Vec<Uuid>>,
    /// Root digit IDs (no parent).
    pub root_ids: Vec<Uuid>,
    /// Container digit → FormationKind mapping (from digit properties or sanctum declarations).
    pub formation_map: HashMap<Uuid, FormationKind>,
    /// Active theme for token resolution.
    pub reign: Reign,
    /// Resolved layout appointments (if available).
    pub appointments: Vec<Appointment>,
}

impl ProjectionContext {
    /// Build the context from a flat list of digits.
    pub fn build(
        digits: &[Digit],
        root_id: Option<Uuid>,
        reign: Reign,
    ) -> Self {
        let mut digit_index = HashMap::new();
        let mut children_index: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut root_ids = Vec::new();
        let mut formation_map = HashMap::new();

        // Index all digits
        for digit in digits {
            digit_index.insert(digit.id(), digit.clone());
        }

        // Build children index from digit.children
        for digit in digits {
            if let Some(children) = &digit.children {
                children_index.insert(digit.id(), children.clone());
            }
        }

        // Identify roots
        if let Some(rid) = root_id {
            root_ids.push(rid);
        } else {
            // Any digit not referenced as a child of another is a root
            let all_children: std::collections::HashSet<Uuid> = children_index
                .values()
                .flat_map(|v| v.iter().copied())
                .collect();
            for digit in digits {
                if !all_children.contains(&digit.id()) {
                    root_ids.push(digit.id());
                }
            }
        }

        // Extract formation declarations from digit properties
        for digit in digits {
            if let Some(formation_val) = digit.properties.get("formation") {
                if let Some(s) = formation_val.as_str() {
                    // Parse simple formation strings
                    let kind = match s {
                        "rank" | "hstack" | "row" => FormationKind::Rank {
                            spacing: 8.0,
                            alignment: regalia::RankAlignment::Center,
                            justification: regalia::RankJustification::Leading,
                        },
                        "column" | "vstack" | "col" => FormationKind::Column {
                            spacing: 8.0,
                            alignment: regalia::ColumnAlignment::Leading,
                            justification: regalia::ColumnJustification::Top,
                        },
                        "tier" | "zstack" | "stack" => FormationKind::Tier,
                        "procession" | "flow" | "wrap" => FormationKind::Procession {
                            horizontal_spacing: 8.0,
                            vertical_spacing: 8.0,
                        },
                        other => FormationKind::Custom(other.into()),
                    };
                    formation_map.insert(digit.id(), kind);
                }
            }
        }

        Self {
            digit_index,
            children_index,
            root_ids,
            formation_map,
            reign,
            appointments: Vec::new(),
        }
    }

    /// Attach resolved layout appointments (from Arbiter).
    pub fn with_appointments(mut self, appointments: Vec<Appointment>) -> Self {
        self.appointments = appointments;
        self
    }

    /// Attach sanctum-based formation declarations.
    pub fn with_sanctum_formations(mut self, sanctums: &[Sanctum], digit_ids: &[Uuid]) -> Self {
        // Map each digit to its sanctum's formation (by position)
        for (i, sanctum) in sanctums.iter().enumerate() {
            if let Some(id) = digit_ids.get(i) {
                self.formation_map.insert(*id, sanctum.formation_kind.clone());
            }
        }
        self
    }

    /// Look up a digit by ID.
    pub fn digit(&self, id: Uuid) -> Option<&Digit> {
        self.digit_index.get(&id)
    }

    /// Get the ordered child IDs of a digit (empty slice if no children).
    pub fn children_of(&self, id: Uuid) -> &[Uuid] {
        self.children_index.get(&id).map_or(&[], |v| v.as_slice())
    }

    /// Get the formation kind declared for a digit (from properties or sanctum).
    pub fn formation_for(&self, digit_id: Uuid) -> Option<&FormationKind> {
        self.formation_map.get(&digit_id)
    }

    /// Resolve the active color palette from the theme.
    pub fn crest(&self) -> &regalia::Crest {
        self.reign.crest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn build_empty() {
        let ctx = ProjectionContext::build(&[], None, Reign::default());
        assert!(ctx.digit_index.is_empty());
        assert!(ctx.root_ids.is_empty());
    }

    #[test]
    fn build_with_parent_child() {
        let parent = make_digit("container");
        let child = make_digit("text");
        let parent_with_child = parent.with_child(child.id(), "cpub1test");
        let pid = parent_with_child.id();
        let cid = child.id();
        let ctx = ProjectionContext::build(
            &[parent_with_child, child],
            Some(pid),
            Reign::default(),
        );
        assert_eq!(ctx.children_of(pid), &[cid]);
        assert_eq!(ctx.root_ids, vec![pid]);
    }

    #[test]
    fn root_detection_without_explicit() {
        let a = make_digit("text");
        let b = make_digit("text");
        let ctx = ProjectionContext::build(&[a.clone(), b.clone()], None, Reign::default());
        assert_eq!(ctx.root_ids.len(), 2);
    }

    #[test]
    fn children_of_leaf_is_empty() {
        let d = make_digit("text");
        let ctx = ProjectionContext::build(std::slice::from_ref(&d), Some(d.id()), Reign::default());
        assert!(ctx.children_of(d.id()).is_empty());
    }

    #[test]
    fn crest_resolves_from_reign() {
        let ctx = ProjectionContext::build(&[], None, Reign::default());
        let _crest = ctx.crest(); // should not panic
    }

    #[test]
    fn formation_from_digit_property() {
        let d = make_digit("container")
            .with_property("formation".into(), Value::from("rank"), "cpub1test");
        let id = d.id();
        let ctx = ProjectionContext::build(&[d], Some(id), Reign::default());
        let fk = ctx.formation_for(id).unwrap();
        assert!(matches!(fk, FormationKind::Rank { .. }));
    }
}
