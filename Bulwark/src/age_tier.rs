use serde::{Deserialize, Serialize};

/// Age-based capability tiers — gradual, not a hard switch at 18.
///
/// Kids (<13) → Teen (13-17) → YoungAdult (18-24) → Adult (25+).
/// Configurable within ranges by community.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgeTier {
    /// Under 13 (default). Protected in Kids Sphere.
    Kid,
    /// 13-17 (default). More autonomy, still protected.
    Teen,
    /// 18-24 (default). Full adult access, some restrictions (e.g., can't sponsor until 25).
    YoungAdult,
    /// 25+ (default). Full capabilities.
    Adult,
}

impl AgeTier {
    /// Whether this tier has access to the adult sphere (YoungAdult or Adult).
    pub fn can_access_adult_sphere(&self) -> bool {
        matches!(self, AgeTier::YoungAdult | AgeTier::Adult)
    }

    /// Whether this tier is in the Kids Sphere (Kid or Teen).
    pub fn is_in_kids_sphere(&self) -> bool {
        matches!(self, AgeTier::Kid | AgeTier::Teen)
    }

    /// Whether this tier can potentially vouch for others (YoungAdult or Adult).
    pub fn can_potentially_vouch(&self) -> bool {
        matches!(self, AgeTier::YoungAdult | AgeTier::Adult)
    }

    /// Whether this tier can sponsor others (Adult only).
    pub fn can_sponsor(&self) -> bool {
        *self == AgeTier::Adult
    }

    /// Determine tier from age in years.
    pub fn from_age(age: u8, config: &AgeTierConfig) -> Self {
        if age <= config.kid_max_age {
            AgeTier::Kid
        } else if age <= config.teen_max_age {
            AgeTier::Teen
        } else if age <= config.young_adult_max_age {
            AgeTier::YoungAdult
        } else {
            AgeTier::Adult
        }
    }
}

/// Configurable age thresholds — communities can adjust within ranges.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgeTierConfig {
    /// Max age for Kid tier (default 12, range 10-13).
    pub kid_max_age: u8,
    /// Max age for Teen tier (default 17, range 15-18).
    pub teen_max_age: u8,
    /// Max age for YoungAdult tier (default 24, range 20-25).
    pub young_adult_max_age: u8,
}

impl AgeTierConfig {
    /// Create a config with custom age boundaries (clamped to safe ranges).
    pub fn new(kid_max: u8, teen_max: u8, young_adult_max: u8) -> Self {
        Self {
            kid_max_age: kid_max.clamp(10, 13),
            teen_max_age: teen_max.clamp(15, 18),
            young_adult_max_age: young_adult_max.clamp(20, 25),
        }
    }

    /// Validate that the config is internally consistent.
    pub fn is_valid(&self) -> bool {
        self.kid_max_age < self.teen_max_age && self.teen_max_age < self.young_adult_max_age
    }
}

impl Default for AgeTierConfig {
    fn default() -> Self {
        Self {
            kid_max_age: 12,
            teen_max_age: 17,
            young_adult_max_age: 24,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_ordering() {
        assert!(AgeTier::Kid < AgeTier::Teen);
        assert!(AgeTier::Teen < AgeTier::YoungAdult);
        assert!(AgeTier::YoungAdult < AgeTier::Adult);
    }

    #[test]
    fn tier_from_age_defaults() {
        let config = AgeTierConfig::default();
        assert_eq!(AgeTier::from_age(5, &config), AgeTier::Kid);
        assert_eq!(AgeTier::from_age(12, &config), AgeTier::Kid);
        assert_eq!(AgeTier::from_age(13, &config), AgeTier::Teen);
        assert_eq!(AgeTier::from_age(17, &config), AgeTier::Teen);
        assert_eq!(AgeTier::from_age(18, &config), AgeTier::YoungAdult);
        assert_eq!(AgeTier::from_age(24, &config), AgeTier::YoungAdult);
        assert_eq!(AgeTier::from_age(25, &config), AgeTier::Adult);
        assert_eq!(AgeTier::from_age(80, &config), AgeTier::Adult);
    }

    #[test]
    fn tier_from_age_custom_config() {
        let config = AgeTierConfig::new(10, 15, 20);
        assert_eq!(AgeTier::from_age(10, &config), AgeTier::Kid);
        assert_eq!(AgeTier::from_age(11, &config), AgeTier::Teen);
        assert_eq!(AgeTier::from_age(16, &config), AgeTier::YoungAdult);
        assert_eq!(AgeTier::from_age(21, &config), AgeTier::Adult);
    }

    #[test]
    fn sphere_access() {
        assert!(AgeTier::Kid.is_in_kids_sphere());
        assert!(AgeTier::Teen.is_in_kids_sphere());
        assert!(!AgeTier::YoungAdult.is_in_kids_sphere());
        assert!(!AgeTier::Adult.is_in_kids_sphere());

        assert!(!AgeTier::Kid.can_access_adult_sphere());
        assert!(!AgeTier::Teen.can_access_adult_sphere());
        assert!(AgeTier::YoungAdult.can_access_adult_sphere());
        assert!(AgeTier::Adult.can_access_adult_sphere());
    }

    #[test]
    fn sponsorship_only_adults() {
        assert!(!AgeTier::Kid.can_sponsor());
        assert!(!AgeTier::Teen.can_sponsor());
        assert!(!AgeTier::YoungAdult.can_sponsor());
        assert!(AgeTier::Adult.can_sponsor());
    }

    #[test]
    fn config_clamping() {
        let config = AgeTierConfig::new(5, 30, 50);
        assert_eq!(config.kid_max_age, 10); // clamped to min 10
        assert_eq!(config.teen_max_age, 18); // clamped to max 18
        assert_eq!(config.young_adult_max_age, 25); // clamped to max 25
    }

    #[test]
    fn config_validity() {
        let valid = AgeTierConfig::default();
        assert!(valid.is_valid());

        // Invalid: teen_max <= kid_max after clamping
        let invalid = AgeTierConfig::new(13, 13, 20);
        // kid_max=13, teen_max=15 (clamped), so actually valid after clamping
        assert!(invalid.is_valid());
    }
}
