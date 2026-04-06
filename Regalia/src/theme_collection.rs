use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::RegaliaError;
use crate::reign::Reign;

/// A collection of themes with an active selection.
///
/// Supports multiple Reigns per user — switch between them at runtime.
/// Serializable for persistence alongside user preferences.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeCollection {
    themes: HashMap<String, Reign>,
    active: String,
}

impl ThemeCollection {
    /// Create a collection with an initial theme. The initial theme becomes the active one.
    pub fn new(initial: Reign) -> Self {
        let name = initial.name.clone();
        let mut themes = HashMap::new();
        themes.insert(name.clone(), initial);
        Self {
            themes,
            active: name,
        }
    }

    /// The currently active theme.
    pub fn active(&self) -> &Reign {
        // Safe: we enforce that `active` always exists in the map —
        // `new()` inserts the initial theme and `switch()` validates the key.
        self.themes
            .get(&self.active)
            .expect("active theme must exist in the map (structural invariant)")
    }

    /// The name of the currently active theme.
    pub fn active_name(&self) -> &str {
        &self.active
    }

    /// Switch to a different theme by name.
    pub fn switch(&mut self, name: &str) -> Result<(), RegaliaError> {
        if self.themes.contains_key(name) {
            self.active = name.to_string();
            Ok(())
        } else {
            Err(RegaliaError::ThemeNotFound(name.to_string()))
        }
    }

    /// List all theme names in the collection.
    pub fn list(&self) -> Vec<&str> {
        self.themes.keys().map(|k| k.as_str()).collect()
    }

    /// Add a new theme. Errors if a theme with this name already exists.
    pub fn add(&mut self, reign: Reign) -> Result<(), RegaliaError> {
        if self.themes.contains_key(&reign.name) {
            return Err(RegaliaError::ThemeAlreadyExists(reign.name.clone()));
        }
        self.themes.insert(reign.name.clone(), reign);
        Ok(())
    }

    /// Remove a theme by name. Errors if it is the active theme or the last theme.
    pub fn remove(&mut self, name: &str) -> Result<Reign, RegaliaError> {
        if self.themes.len() == 1 {
            return Err(RegaliaError::CannotRemoveLastTheme);
        }
        if self.active == name {
            return Err(RegaliaError::CannotRemoveActiveTheme(name.to_string()));
        }
        self.themes
            .remove(name)
            .ok_or_else(|| RegaliaError::ThemeNotFound(name.to_string()))
    }

    /// Get an immutable reference to a theme by name.
    pub fn get(&self, name: &str) -> Option<&Reign> {
        self.themes.get(name)
    }

    /// Get a mutable reference to a theme by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Reign> {
        self.themes.get_mut(name)
    }

    /// The number of themes in the collection.
    pub fn count(&self) -> usize {
        self.themes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aura::Aura;
    use crate::reign::Aspect;

    fn make_reign(name: &str) -> Reign {
        Reign::new(name, Aura::default(), Aspect::light())
    }

    #[test]
    fn new_collection() {
        let tc = ThemeCollection::new(make_reign("Default"));
        assert_eq!(tc.active_name(), "Default");
        assert_eq!(tc.count(), 1);
    }

    #[test]
    fn active_returns_initial() {
        let tc = ThemeCollection::new(make_reign("Ocean"));
        assert_eq!(tc.active().name, "Ocean");
    }

    #[test]
    fn add_and_switch() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.add(make_reign("Dark")).unwrap();
        assert_eq!(tc.count(), 2);
        tc.switch("Dark").unwrap();
        assert_eq!(tc.active_name(), "Dark");
    }

    #[test]
    fn switch_nonexistent() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        let result = tc.switch("Ghost");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegaliaError::ThemeNotFound(_)
        ));
    }

    #[test]
    fn add_duplicate() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        let result = tc.add(make_reign("Default"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegaliaError::ThemeAlreadyExists(_)
        ));
    }

    #[test]
    fn remove_inactive_theme() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.add(make_reign("Sunset")).unwrap();
        let removed = tc.remove("Sunset").unwrap();
        assert_eq!(removed.name, "Sunset");
        assert_eq!(tc.count(), 1);
    }

    #[test]
    fn cannot_remove_active() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.add(make_reign("Other")).unwrap();
        let result = tc.remove("Default");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegaliaError::CannotRemoveActiveTheme(_)
        ));
    }

    #[test]
    fn cannot_remove_last() {
        let mut tc = ThemeCollection::new(make_reign("Only"));
        let result = tc.remove("Only");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegaliaError::CannotRemoveLastTheme
        ));
    }

    #[test]
    fn remove_nonexistent() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.add(make_reign("Other")).unwrap();
        let result = tc.remove("Ghost");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegaliaError::ThemeNotFound(_)
        ));
    }

    #[test]
    fn list_themes() {
        let mut tc = ThemeCollection::new(make_reign("Alpha"));
        tc.add(make_reign("Beta")).unwrap();
        tc.add(make_reign("Gamma")).unwrap();
        let names = tc.list();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"Alpha"));
        assert!(names.contains(&"Beta"));
        assert!(names.contains(&"Gamma"));
    }

    #[test]
    fn get_and_get_mut() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        assert!(tc.get("Default").is_some());
        assert!(tc.get("Missing").is_none());
        assert!(tc.get_mut("Default").is_some());
        assert!(tc.get_mut("Missing").is_none());
    }

    #[test]
    fn get_mut_modifies() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.get_mut("Default").unwrap().aspect = Aspect::dark();
        assert_eq!(tc.active().aspect, Aspect::dark());
    }

    #[test]
    fn serde_roundtrip() {
        let mut tc = ThemeCollection::new(make_reign("Default"));
        tc.add(make_reign("Dark")).unwrap();
        let json = serde_json::to_string(&tc).unwrap();
        let decoded: ThemeCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.active_name(), "Default");
        assert_eq!(decoded.count(), 2);
        assert!(decoded.get("Dark").is_some());
    }

    #[test]
    fn count() {
        let mut tc = ThemeCollection::new(make_reign("A"));
        assert_eq!(tc.count(), 1);
        tc.add(make_reign("B")).unwrap();
        assert_eq!(tc.count(), 2);
        tc.add(make_reign("C")).unwrap();
        assert_eq!(tc.count(), 3);
    }
}
