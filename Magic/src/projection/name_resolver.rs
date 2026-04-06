use std::collections::HashSet;

/// Resolves names for code generation. Handles casing, deduplication,
/// and reserved word escaping.
#[derive(Debug, Clone)]
pub struct NameResolver {
    used_names: HashSet<String>,
    reserved_words: HashSet<String>,
}

impl NameResolver {
    /// Create a resolver with no reserved words.
    pub fn new() -> Self {
        Self {
            used_names: HashSet::new(),
            reserved_words: HashSet::new(),
        }
    }

    /// Create a resolver pre-loaded with Swift reserved words for backtick escaping.
    pub fn with_swift_reserved() -> Self {
        let words = [
            "class", "struct", "enum", "protocol", "extension", "func", "var", "let", "if",
            "else", "for", "while", "do", "switch", "case", "default", "break", "continue",
            "return", "throw", "try", "catch", "import", "typealias", "associatedtype", "init",
            "deinit", "subscript", "operator", "precedencegroup", "where", "guard", "defer",
            "repeat", "in", "as", "is", "self", "Self", "super", "true", "false", "nil", "Type",
            "Protocol", "Any", "AnyObject", "some", "async", "await", "actor",
        ];
        Self {
            used_names: HashSet::new(),
            reserved_words: words.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// PascalCase, deduplicated (e.g. "my-widget" → "MyWidget").
    pub fn type_name(&mut self, raw: &str) -> String {
        let name = Self::to_pascal_case(raw);
        let name = self.escape_reserved(&name);
        self.deduplicate(name)
    }

    /// camelCase, deduplicated (e.g. "Button Label" → "buttonLabel").
    pub fn property_name(&mut self, raw: &str) -> String {
        let name = Self::to_camel_case(raw);
        let name = self.escape_reserved(&name);
        self.deduplicate(name)
    }

    /// kebab-case (e.g. "Profile Pic" → "profile-pic").
    pub fn asset_name(raw: &str) -> String {
        Self::to_kebab_case(raw)
    }

    /// SCREAMING_SNAKE_CASE (e.g. "max size" → "MAX_SIZE").
    pub fn constant_name(raw: &str) -> String {
        Self::to_screaming_snake(raw)
    }

    /// Reset deduplication tracking.
    pub fn reset(&mut self) {
        self.used_names.clear();
    }

    // --- Case transforms (pure functions) ---

    pub fn to_pascal_case(s: &str) -> String {
        Self::split_words(s)
            .iter()
            .map(|w| Self::capitalize(w))
            .collect()
    }

    pub fn to_camel_case(s: &str) -> String {
        let words = Self::split_words(s);
        if words.is_empty() {
            return String::new();
        }
        let mut result = words[0].to_lowercase();
        for w in &words[1..] {
            result.push_str(&Self::capitalize(w));
        }
        result
    }

    pub fn to_kebab_case(s: &str) -> String {
        Self::split_words(s)
            .iter()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join("-")
    }

    pub fn to_screaming_snake(s: &str) -> String {
        Self::split_words(s)
            .iter()
            .map(|w| w.to_uppercase())
            .collect::<Vec<_>>()
            .join("_")
    }

    // --- Internal ---

    fn split_words(s: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();

        for ch in s.chars() {
            if ch == '-' || ch == '_' || ch == '/' || ch == ' ' || ch == '.' {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
            } else if ch.is_uppercase() && !current.is_empty() && !current.ends_with(char::is_uppercase) {
                // camelCase boundary: lowercase followed by uppercase
                words.push(current.clone());
                current.clear();
                current.push(ch);
            } else if ch.is_alphanumeric() {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            words.push(current);
        }
        words
    }

    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
        }
    }

    fn deduplicate(&mut self, name: String) -> String {
        if !self.used_names.contains(&name) {
            self.used_names.insert(name.clone());
            return name;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{name}{suffix}");
            if !self.used_names.contains(&candidate) {
                self.used_names.insert(candidate.clone());
                return candidate;
            }
            suffix += 1;
        }
    }

    fn escape_reserved(&self, name: &str) -> String {
        if self.reserved_words.contains(name) {
            format!("`{name}`")
        } else if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            format!("N{name}")
        } else {
            name.to_string()
        }
    }
}

impl Default for NameResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_basic() {
        assert_eq!(NameResolver::to_pascal_case("my-cool widget"), "MyCoolWidget");
        assert_eq!(NameResolver::to_pascal_case("button"), "Button");
    }

    #[test]
    fn camel_case_basic() {
        assert_eq!(NameResolver::to_camel_case("Button Label"), "buttonLabel");
        assert_eq!(NameResolver::to_camel_case("x"), "x");
    }

    #[test]
    fn kebab_case() {
        assert_eq!(NameResolver::to_kebab_case("Profile Pic"), "profile-pic");
        assert_eq!(NameResolver::to_kebab_case("MyCoolThing"), "my-cool-thing");
    }

    #[test]
    fn screaming_snake() {
        assert_eq!(NameResolver::to_screaming_snake("max size"), "MAX_SIZE");
        assert_eq!(NameResolver::to_screaming_snake("api-key"), "API_KEY");
    }

    #[test]
    fn deduplication() {
        let mut r = NameResolver::new();
        assert_eq!(r.type_name("home"), "Home");
        assert_eq!(r.type_name("home"), "Home2");
        assert_eq!(r.type_name("home"), "Home3");
    }

    #[test]
    fn reserved_word_escaping() {
        let mut r = NameResolver::with_swift_reserved();
        let name = r.property_name("default");
        assert_eq!(name, "`default`");
    }

    #[test]
    fn mixed_separators() {
        assert_eq!(NameResolver::to_pascal_case("my-cool_name"), "MyCoolName");
        assert_eq!(NameResolver::to_pascal_case("Buttons/Primary"), "ButtonsPrimary");
    }

    #[test]
    fn empty_string() {
        assert_eq!(NameResolver::to_pascal_case(""), "");
        assert_eq!(NameResolver::to_camel_case(""), "");
    }

    #[test]
    fn single_character() {
        assert_eq!(NameResolver::to_pascal_case("a"), "A");
        assert_eq!(NameResolver::to_camel_case("A"), "a");
    }

    #[test]
    fn reset_clears_deduplication() {
        let mut r = NameResolver::new();
        assert_eq!(r.type_name("home"), "Home");
        assert_eq!(r.type_name("home"), "Home2");
        r.reset();
        assert_eq!(r.type_name("home"), "Home");
    }
}
