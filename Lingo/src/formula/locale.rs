use std::collections::HashMap;

/// Locale-specific formula display settings.
///
/// Handles function name translation and number formatting differences
/// across languages. Formulas are stored in canonical (English) form
/// and translated for display.
pub struct FormulaLocale {
    /// Canonical English name -> localized name.
    function_names: HashMap<String, String>,
    /// Localized name -> canonical English name.
    reverse_names: HashMap<String, String>,
    /// Decimal separator character (`.` for English, `,` for many European languages).
    pub decimal_separator: char,
    /// Argument separator character (`,` for English, `;` for languages using `,` as decimal).
    pub argument_separator: char,
}

impl FormulaLocale {
    /// English (canonical) locale — function names are identity-mapped.
    pub fn english() -> Self {
        Self {
            function_names: HashMap::new(),
            reverse_names: HashMap::new(),
            decimal_separator: '.',
            argument_separator: ',',
        }
    }

    /// French locale with translated function names.
    pub fn french() -> Self {
        let translations = vec![
            ("SUM", "SOMME"),
            ("AVERAGE", "MOYENNE"),
            ("COUNT", "NB"),
            ("COUNTIF", "NB.SI"),
            ("SUMIF", "SOMME.SI"),
            ("MIN", "MIN"),
            ("MAX", "MAX"),
            ("ROUND", "ARRONDI"),
            ("ABS", "ABS"),
            ("MOD", "MOD"),
            ("POWER", "PUISSANCE"),
            ("CONCAT", "CONCATENER"),
            ("LEFT", "GAUCHE"),
            ("RIGHT", "DROITE"),
            ("MID", "STXT"),
            ("LEN", "NBCAR"),
            ("UPPER", "MAJUSCULE"),
            ("LOWER", "MINUSCULE"),
            ("TRIM", "SUPPRESPACE"),
            ("IF", "SI"),
            ("AND", "ET"),
            ("OR", "OU"),
            ("NOT", "NON"),
        ];

        Self::from_translations(translations, ',', ';')
    }

    /// German locale with translated function names.
    pub fn german() -> Self {
        let translations = vec![
            ("SUM", "SUMME"),
            ("AVERAGE", "MITTELWERT"),
            ("COUNT", "ANZAHL"),
            ("COUNTIF", "ZAEHLENWENN"),
            ("SUMIF", "SUMMEWENN"),
            ("MIN", "MIN"),
            ("MAX", "MAX"),
            ("ROUND", "RUNDEN"),
            ("ABS", "ABS"),
            ("MOD", "REST"),
            ("POWER", "POTENZ"),
            ("CONCAT", "VERKETTEN"),
            ("LEFT", "LINKS"),
            ("RIGHT", "RECHTS"),
            ("MID", "TEIL"),
            ("LEN", "LAENGE"),
            ("UPPER", "GROSS"),
            ("LOWER", "KLEIN"),
            ("TRIM", "GLAETTEN"),
            ("IF", "WENN"),
            ("AND", "UND"),
            ("OR", "ODER"),
            ("NOT", "NICHT"),
        ];

        Self::from_translations(translations, ',', ';')
    }

    /// Look up the localized name for a canonical function name.
    /// Returns the canonical name if no translation exists.
    pub fn localize_name<'a>(&'a self, canonical: &'a str) -> &'a str {
        self.function_names
            .get(&canonical.to_uppercase())
            .map(|s| s.as_str())
            .unwrap_or(canonical)
    }

    /// Look up the canonical name for a localized function name.
    /// Returns `None` if the name is unknown.
    pub fn canonicalize_name<'a>(&'a self, localized: &'a str) -> Option<&'a str> {
        let upper = localized.to_uppercase();
        // First check if it's already canonical (untranslated).
        if self.function_names.is_empty() || self.function_names.contains_key(&upper) {
            return Some(localized);
        }
        self.reverse_names.get(&upper).map(|s| s.as_str())
    }

    /// Convert a canonical formula string to localized display form.
    ///
    /// Translates function names and adjusts separators.
    pub fn to_display(&self, formula: &str) -> String {
        let mut result = String::with_capacity(formula.len());
        let chars: Vec<char> = formula.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i].is_ascii_alphabetic() {
                // Collect identifier
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                // Check if followed by ( — it's a function name
                let is_function = i < chars.len() && chars[i] == '(';
                if is_function {
                    let localized = self.localize_name(&ident);
                    result.push_str(localized);
                } else {
                    result.push_str(&ident);
                }
            } else if chars[i] == ',' {
                // Replace argument separator
                result.push(self.argument_separator);
                i += 1;
            } else if chars[i] == '.' && self.decimal_separator != '.' {
                // Check if this is inside a number (digit before and after)
                let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let next_is_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
                if prev_is_digit && next_is_digit {
                    result.push(self.decimal_separator);
                } else {
                    result.push('.');
                }
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    /// Convert a localized formula display string to canonical (English) form
    /// for storage.
    pub fn to_canonical(&self, display: &str) -> String {
        let mut result = String::with_capacity(display.len());
        let chars: Vec<char> = display.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i].is_ascii_alphabetic() {
                // Collect identifier (including dots for function names like NB.SI)
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                // Check if followed by ( — it's a function name
                let is_function = i < chars.len() && chars[i] == '(';
                if is_function {
                    if let Some(canonical) = self.canonicalize_name(&ident) {
                        result.push_str(canonical);
                    } else {
                        result.push_str(&ident);
                    }
                } else {
                    result.push_str(&ident);
                }
            } else if chars[i] == self.argument_separator && self.argument_separator != ',' {
                result.push(',');
                i += 1;
            } else if chars[i] == self.decimal_separator && self.decimal_separator != '.' {
                // Check if this is inside a number (digit before and after)
                let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let next_is_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
                if prev_is_digit && next_is_digit {
                    result.push('.');
                } else {
                    result.push(chars[i]);
                }
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    // --- Private helpers ---

    fn from_translations(
        translations: Vec<(&str, &str)>,
        decimal_separator: char,
        argument_separator: char,
    ) -> Self {
        let mut function_names = HashMap::new();
        let mut reverse_names = HashMap::new();

        for (canonical, localized) in translations {
            function_names.insert(canonical.to_uppercase(), localized.to_uppercase());
            reverse_names.insert(localized.to_uppercase(), canonical.to_uppercase());
        }

        Self {
            function_names,
            reverse_names,
            decimal_separator,
            argument_separator,
        }
    }
}

impl Default for FormulaLocale {
    fn default() -> Self {
        Self::english()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_identity() {
        let locale = FormulaLocale::english();
        assert_eq!(locale.localize_name("SUM"), "SUM");
        assert_eq!(locale.decimal_separator, '.');
        assert_eq!(locale.argument_separator, ',');
    }

    #[test]
    fn french_translations() {
        let locale = FormulaLocale::french();
        assert_eq!(locale.localize_name("SUM"), "SOMME");
        assert_eq!(locale.localize_name("AVERAGE"), "MOYENNE");
        assert_eq!(locale.localize_name("IF"), "SI");
        assert_eq!(locale.localize_name("AND"), "ET");
        assert_eq!(locale.localize_name("COUNT"), "NB");
    }

    #[test]
    fn german_translations() {
        let locale = FormulaLocale::german();
        assert_eq!(locale.localize_name("SUM"), "SUMME");
        assert_eq!(locale.localize_name("AVERAGE"), "MITTELWERT");
        assert_eq!(locale.localize_name("IF"), "WENN");
        assert_eq!(locale.localize_name("AND"), "UND");
    }

    #[test]
    fn french_round_trip() {
        let locale = FormulaLocale::french();

        // Canonical -> French
        let canonical = "=SI(A1>0, SOMME(B1:B10), 0)";
        // But actually, we'd start from English canonical:
        let english = "=IF(A1>0, SUM(B1:B10), 0)";
        let display = locale.to_display(english);
        assert!(display.contains("SI("));
        assert!(display.contains("SOMME("));

        // Back to canonical
        let back = locale.to_canonical(&display);
        assert!(back.contains("IF("));
        assert!(back.contains("SUM("));

        // Ignore the canonical variable to avoid unused warning
        let _ = canonical;
    }

    #[test]
    fn german_round_trip() {
        let locale = FormulaLocale::german();

        let english = "=WENN(A1>0, SUMME(B1:B10), 0)";
        let _ = english; // This is already German, let's test properly:

        let canonical = "=IF(A1>0, SUM(B1:B10), 0)";
        let display = locale.to_display(canonical);
        assert!(display.contains("WENN("));
        assert!(display.contains("SUMME("));

        let back = locale.to_canonical(&display);
        assert!(back.contains("IF("));
        assert!(back.contains("SUM("));
    }

    #[test]
    fn canonicalize_unknown_name() {
        let locale = FormulaLocale::french();
        // Unknown localized name returns None
        assert!(locale.canonicalize_name("XYZZY").is_none());
    }

    #[test]
    fn separator_replacement() {
        let locale = FormulaLocale::french();
        let english = "=SUM(1.5, 2.5)";
        let display = locale.to_display(english);
        // Should use ; as argument separator and , as decimal
        assert!(display.contains(';'));
        assert!(display.contains("1,5"));
        assert!(display.contains("2,5"));
    }

    #[test]
    fn separator_round_trip() {
        let locale = FormulaLocale::french();
        let english = "=IF(A1>1.5, SUM(B1:B10), 0)";
        let display = locale.to_display(english);
        let back = locale.to_canonical(&display);
        // After round-trip, should match original structure
        assert!(back.contains(","));
        assert!(back.contains("1.5"));
    }

    #[test]
    fn english_is_noop() {
        let locale = FormulaLocale::english();
        let formula = "=IF(A1>0, SUM(B1:B10), 0)";
        assert_eq!(locale.to_display(formula), formula);
        assert_eq!(locale.to_canonical(formula), formula);
    }

    #[test]
    fn default_is_english() {
        let locale = FormulaLocale::default();
        assert_eq!(locale.decimal_separator, '.');
        assert_eq!(locale.argument_separator, ',');
    }

    #[test]
    fn case_insensitive_lookup() {
        let locale = FormulaLocale::french();
        assert_eq!(locale.localize_name("sum"), "SOMME");
        assert!(locale.canonicalize_name("somme").is_some());
    }
}
