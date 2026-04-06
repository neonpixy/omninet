use std::collections::HashMap;

use super::value::{FormulaErrorKind, FormulaValue};

/// A function implementation that takes a slice of evaluated arguments
/// and returns a single formula value.
pub type FormulaFn = fn(&[FormulaValue]) -> FormulaValue;

/// Registry of available spreadsheet functions.
///
/// Maps canonical uppercase function names (e.g., "SUM", "IF") to their
/// implementations and expected argument counts. Use [`with_defaults`](FunctionRegistry::with_defaults)
/// for the 23 built-in functions, or build a custom registry with [`register`](FunctionRegistry::register).
pub struct FunctionRegistry {
    /// Maps canonical uppercase names to (implementation, expected_arg_count).
    /// `None` for arg_count means variadic.
    functions: HashMap<String, (FormulaFn, Option<usize>)>,
}

impl FunctionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with all built-in functions.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();

        // Math functions
        reg.register("SUM", fn_sum, None);
        reg.register("AVERAGE", fn_average, None);
        reg.register("MIN", fn_min, None);
        reg.register("MAX", fn_max, None);
        reg.register("COUNT", fn_count, None);
        reg.register("ROUND", fn_round, Some(2));
        reg.register("ABS", fn_abs, Some(1));
        reg.register("MOD", fn_mod, Some(2));
        reg.register("POWER", fn_power, Some(2));

        // Text functions
        reg.register("CONCAT", fn_concat, None);
        reg.register("LEFT", fn_left, Some(2));
        reg.register("RIGHT", fn_right, Some(2));
        reg.register("MID", fn_mid, Some(3));
        reg.register("LEN", fn_len, Some(1));
        reg.register("UPPER", fn_upper, Some(1));
        reg.register("LOWER", fn_lower, Some(1));
        reg.register("TRIM", fn_trim, Some(1));

        // Logic functions
        reg.register("IF", fn_if, Some(3));
        reg.register("AND", fn_and, None);
        reg.register("OR", fn_or, None);
        reg.register("NOT", fn_not, Some(1));

        // Aggregate functions
        reg.register("COUNTIF", fn_countif, Some(2));
        reg.register("SUMIF", fn_sumif, Some(2));

        reg
    }

    /// Register a function with an optional expected argument count.
    pub fn register(&mut self, name: &str, func: FormulaFn, arg_count: Option<usize>) {
        self.functions
            .insert(name.to_uppercase(), (func, arg_count));
    }

    /// Look up a function by canonical name.
    pub fn get(&self, name: &str) -> Option<&(FormulaFn, Option<usize>)> {
        self.functions.get(&name.to_uppercase())
    }

    /// Check if a function exists.
    pub fn has(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---- Math functions ----

fn fn_sum(args: &[FormulaValue]) -> FormulaValue {
    let mut total = 0.0;
    for arg in args {
        match arg {
            FormulaValue::Number(n) => total += n,
            FormulaValue::Bool(b) => total += if *b { 1.0 } else { 0.0 },
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            FormulaValue::Empty | FormulaValue::Text(_) => {} // skip
            FormulaValue::Date(_) => {}
        }
    }
    FormulaValue::Number(total)
}

fn fn_average(args: &[FormulaValue]) -> FormulaValue {
    let mut total = 0.0;
    let mut count = 0usize;
    for arg in args {
        match arg {
            FormulaValue::Number(n) => {
                total += n;
                count += 1;
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            _ => {}
        }
    }
    if count == 0 {
        FormulaValue::Error(FormulaErrorKind::Div0)
    } else {
        FormulaValue::Number(total / count as f64)
    }
}

fn fn_min(args: &[FormulaValue]) -> FormulaValue {
    let mut min: Option<f64> = None;
    for arg in args {
        match arg {
            FormulaValue::Number(n) => {
                min = Some(match min {
                    Some(current) => current.min(*n),
                    None => *n,
                });
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            _ => {}
        }
    }
    match min {
        Some(n) => FormulaValue::Number(n),
        None => FormulaValue::Number(0.0),
    }
}

fn fn_max(args: &[FormulaValue]) -> FormulaValue {
    let mut max: Option<f64> = None;
    for arg in args {
        match arg {
            FormulaValue::Number(n) => {
                max = Some(match max {
                    Some(current) => current.max(*n),
                    None => *n,
                });
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            _ => {}
        }
    }
    match max {
        Some(n) => FormulaValue::Number(n),
        None => FormulaValue::Number(0.0),
    }
}

fn fn_count(args: &[FormulaValue]) -> FormulaValue {
    let count = args
        .iter()
        .filter(|v| matches!(v, FormulaValue::Number(_)))
        .count();
    FormulaValue::Number(count as f64)
}

fn fn_round(args: &[FormulaValue]) -> FormulaValue {
    let (num, digits) = match (args.first(), args.get(1)) {
        (Some(FormulaValue::Number(n)), Some(FormulaValue::Number(d))) => (*n, *d as i32),
        (Some(FormulaValue::Error(e)), _) | (_, Some(FormulaValue::Error(e))) => {
            return FormulaValue::Error(e.clone());
        }
        _ => return FormulaValue::Error(FormulaErrorKind::Value),
    };

    let factor = 10f64.powi(digits);
    FormulaValue::Number((num * factor).round() / factor)
}

fn fn_abs(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Number(n)) => FormulaValue::Number(n.abs()),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_mod(args: &[FormulaValue]) -> FormulaValue {
    match (args.first(), args.get(1)) {
        (Some(FormulaValue::Number(a)), Some(FormulaValue::Number(b))) => {
            if *b == 0.0 {
                FormulaValue::Error(FormulaErrorKind::Div0)
            } else {
                FormulaValue::Number(a % b)
            }
        }
        (Some(FormulaValue::Error(e)), _) | (_, Some(FormulaValue::Error(e))) => {
            FormulaValue::Error(e.clone())
        }
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_power(args: &[FormulaValue]) -> FormulaValue {
    match (args.first(), args.get(1)) {
        (Some(FormulaValue::Number(base)), Some(FormulaValue::Number(exp))) => {
            FormulaValue::Number(base.powf(*exp))
        }
        (Some(FormulaValue::Error(e)), _) | (_, Some(FormulaValue::Error(e))) => {
            FormulaValue::Error(e.clone())
        }
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

// ---- Text functions ----

fn fn_concat(args: &[FormulaValue]) -> FormulaValue {
    let mut result = String::new();
    for arg in args {
        match arg {
            FormulaValue::Text(s) => result.push_str(s),
            FormulaValue::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    result.push_str(&(*n as i64).to_string());
                } else {
                    result.push_str(&n.to_string());
                }
            }
            FormulaValue::Bool(b) => {
                result.push_str(if *b { "TRUE" } else { "FALSE" });
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            FormulaValue::Empty => {}
            FormulaValue::Date(dt) => result.push_str(&dt.to_string()),
        }
    }
    FormulaValue::Text(result)
}

fn fn_left(args: &[FormulaValue]) -> FormulaValue {
    match (args.first(), args.get(1)) {
        (Some(FormulaValue::Text(s)), Some(FormulaValue::Number(n))) => {
            let n = *n as usize;
            let result: String = s.chars().take(n).collect();
            FormulaValue::Text(result)
        }
        (Some(FormulaValue::Error(e)), _) | (_, Some(FormulaValue::Error(e))) => {
            FormulaValue::Error(e.clone())
        }
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_right(args: &[FormulaValue]) -> FormulaValue {
    match (args.first(), args.get(1)) {
        (Some(FormulaValue::Text(s)), Some(FormulaValue::Number(n))) => {
            let n = *n as usize;
            let chars: Vec<char> = s.chars().collect();
            let start = chars.len().saturating_sub(n);
            let result: String = chars[start..].iter().collect();
            FormulaValue::Text(result)
        }
        (Some(FormulaValue::Error(e)), _) | (_, Some(FormulaValue::Error(e))) => {
            FormulaValue::Error(e.clone())
        }
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_mid(args: &[FormulaValue]) -> FormulaValue {
    match (args.first(), args.get(1), args.get(2)) {
        (
            Some(FormulaValue::Text(s)),
            Some(FormulaValue::Number(start)),
            Some(FormulaValue::Number(length)),
        ) => {
            // MID is 1-based like Excel
            let start = (*start as usize).saturating_sub(1);
            let length = *length as usize;
            let chars: Vec<char> = s.chars().collect();
            let end = (start + length).min(chars.len());
            let start = start.min(chars.len());
            let result: String = chars[start..end].iter().collect();
            FormulaValue::Text(result)
        }
        (Some(FormulaValue::Error(e)), _, _)
        | (_, Some(FormulaValue::Error(e)), _)
        | (_, _, Some(FormulaValue::Error(e))) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_len(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Text(s)) => FormulaValue::Number(s.chars().count() as f64),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_upper(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Text(s)) => FormulaValue::Text(s.to_uppercase()),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_lower(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Text(s)) => FormulaValue::Text(s.to_lowercase()),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

fn fn_trim(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Text(s)) => FormulaValue::Text(s.trim().to_string()),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

// ---- Logic functions ----

fn fn_if(args: &[FormulaValue]) -> FormulaValue {
    let condition = match args.first() {
        Some(FormulaValue::Bool(b)) => *b,
        Some(FormulaValue::Number(n)) => *n != 0.0,
        Some(FormulaValue::Error(e)) => return FormulaValue::Error(e.clone()),
        _ => return FormulaValue::Error(FormulaErrorKind::Value),
    };

    if condition {
        args.get(1).cloned().unwrap_or(FormulaValue::Empty)
    } else {
        args.get(2).cloned().unwrap_or(FormulaValue::Empty)
    }
}

fn fn_and(args: &[FormulaValue]) -> FormulaValue {
    for arg in args {
        match arg {
            FormulaValue::Bool(b) => {
                if !b {
                    return FormulaValue::Bool(false);
                }
            }
            FormulaValue::Number(n) => {
                if *n == 0.0 {
                    return FormulaValue::Bool(false);
                }
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            _ => return FormulaValue::Error(FormulaErrorKind::Value),
        }
    }
    FormulaValue::Bool(true)
}

fn fn_or(args: &[FormulaValue]) -> FormulaValue {
    for arg in args {
        match arg {
            FormulaValue::Bool(b) => {
                if *b {
                    return FormulaValue::Bool(true);
                }
            }
            FormulaValue::Number(n) => {
                if *n != 0.0 {
                    return FormulaValue::Bool(true);
                }
            }
            FormulaValue::Error(e) => return FormulaValue::Error(e.clone()),
            _ => return FormulaValue::Error(FormulaErrorKind::Value),
        }
    }
    FormulaValue::Bool(false)
}

fn fn_not(args: &[FormulaValue]) -> FormulaValue {
    match args.first() {
        Some(FormulaValue::Bool(b)) => FormulaValue::Bool(!b),
        Some(FormulaValue::Number(n)) => FormulaValue::Bool(*n == 0.0),
        Some(FormulaValue::Error(e)) => FormulaValue::Error(e.clone()),
        _ => FormulaValue::Error(FormulaErrorKind::Value),
    }
}

// ---- Aggregate functions ----

/// COUNTIF(range_values..., criteria) — counts how many values in range match criteria.
/// The last argument is the criteria (a value to compare against).
/// All preceding arguments are the range values.
fn fn_countif(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaErrorKind::Na);
    }
    let criteria = &args[args.len() - 1];
    let range = &args[..args.len() - 1];

    let count = range
        .iter()
        .filter(|v| values_match(v, criteria))
        .count();
    FormulaValue::Number(count as f64)
}

/// SUMIF(range_values..., criteria) — sums values in range that match criteria.
/// Same convention as COUNTIF: last arg is criteria, rest are range.
fn fn_sumif(args: &[FormulaValue]) -> FormulaValue {
    if args.len() < 2 {
        return FormulaValue::Error(FormulaErrorKind::Na);
    }
    let criteria = &args[args.len() - 1];
    let range = &args[..args.len() - 1];

    let sum: f64 = range
        .iter()
        .filter(|v| values_match(v, criteria))
        .filter_map(|v| v.as_number())
        .sum();
    FormulaValue::Number(sum)
}

/// Simple equality comparison for COUNTIF/SUMIF criteria matching.
fn values_match(value: &FormulaValue, criteria: &FormulaValue) -> bool {
    match (value, criteria) {
        (FormulaValue::Number(a), FormulaValue::Number(b)) => (a - b).abs() < f64::EPSILON,
        (FormulaValue::Text(a), FormulaValue::Text(b)) => a.to_lowercase() == b.to_lowercase(),
        (FormulaValue::Bool(a), FormulaValue::Bool(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_numbers() {
        let result = fn_sum(&[
            FormulaValue::Number(1.0),
            FormulaValue::Number(2.0),
            FormulaValue::Number(3.0),
        ]);
        assert_eq!(result, FormulaValue::Number(6.0));
    }

    #[test]
    fn sum_skips_text() {
        let result = fn_sum(&[
            FormulaValue::Number(1.0),
            FormulaValue::Text("ignored".into()),
            FormulaValue::Number(3.0),
        ]);
        assert_eq!(result, FormulaValue::Number(4.0));
    }

    #[test]
    fn sum_propagates_error() {
        let result = fn_sum(&[
            FormulaValue::Number(1.0),
            FormulaValue::Error(FormulaErrorKind::Ref),
        ]);
        assert!(result.is_error());
    }

    #[test]
    fn average_basic() {
        let result = fn_average(&[
            FormulaValue::Number(2.0),
            FormulaValue::Number(4.0),
            FormulaValue::Number(6.0),
        ]);
        assert_eq!(result, FormulaValue::Number(4.0));
    }

    #[test]
    fn average_empty_is_div0() {
        let result = fn_average(&[]);
        assert_eq!(result, FormulaValue::Error(FormulaErrorKind::Div0));
    }

    #[test]
    fn min_max() {
        let vals = [
            FormulaValue::Number(3.0),
            FormulaValue::Number(1.0),
            FormulaValue::Number(5.0),
        ];
        assert_eq!(fn_min(&vals), FormulaValue::Number(1.0));
        assert_eq!(fn_max(&vals), FormulaValue::Number(5.0));
    }

    #[test]
    fn count_numbers_only() {
        let result = fn_count(&[
            FormulaValue::Number(1.0),
            FormulaValue::Text("hi".into()),
            FormulaValue::Number(2.0),
            FormulaValue::Empty,
        ]);
        assert_eq!(result, FormulaValue::Number(2.0));
    }

    #[test]
    fn round_basic() {
        let result = fn_round(&[FormulaValue::Number(3.456), FormulaValue::Number(2.0)]);
        assert_eq!(result, FormulaValue::Number(3.46));

        let result = fn_round(&[FormulaValue::Number(3.456), FormulaValue::Number(0.0)]);
        assert_eq!(result, FormulaValue::Number(3.0));
    }

    #[test]
    fn abs_basic() {
        assert_eq!(
            fn_abs(&[FormulaValue::Number(-5.0)]),
            FormulaValue::Number(5.0)
        );
        assert_eq!(
            fn_abs(&[FormulaValue::Number(5.0)]),
            FormulaValue::Number(5.0)
        );
    }

    #[test]
    fn mod_basic() {
        assert_eq!(
            fn_mod(&[FormulaValue::Number(10.0), FormulaValue::Number(3.0)]),
            FormulaValue::Number(1.0)
        );
    }

    #[test]
    fn mod_div_zero() {
        let result = fn_mod(&[FormulaValue::Number(10.0), FormulaValue::Number(0.0)]);
        assert_eq!(result, FormulaValue::Error(FormulaErrorKind::Div0));
    }

    #[test]
    fn power_basic() {
        assert_eq!(
            fn_power(&[FormulaValue::Number(2.0), FormulaValue::Number(3.0)]),
            FormulaValue::Number(8.0)
        );
    }

    #[test]
    fn concat_mixed() {
        let result = fn_concat(&[
            FormulaValue::Text("hello ".into()),
            FormulaValue::Number(42.0),
            FormulaValue::Text("!".into()),
        ]);
        assert_eq!(result, FormulaValue::Text("hello 42!".into()));
    }

    #[test]
    fn left_right_mid() {
        assert_eq!(
            fn_left(&[FormulaValue::Text("hello".into()), FormulaValue::Number(3.0)]),
            FormulaValue::Text("hel".into())
        );
        assert_eq!(
            fn_right(&[FormulaValue::Text("hello".into()), FormulaValue::Number(3.0)]),
            FormulaValue::Text("llo".into())
        );
        // MID is 1-based
        assert_eq!(
            fn_mid(&[
                FormulaValue::Text("hello".into()),
                FormulaValue::Number(2.0),
                FormulaValue::Number(3.0)
            ]),
            FormulaValue::Text("ell".into())
        );
    }

    #[test]
    fn len_basic() {
        assert_eq!(
            fn_len(&[FormulaValue::Text("hello".into())]),
            FormulaValue::Number(5.0)
        );
    }

    #[test]
    fn upper_lower_trim() {
        assert_eq!(
            fn_upper(&[FormulaValue::Text("hello".into())]),
            FormulaValue::Text("HELLO".into())
        );
        assert_eq!(
            fn_lower(&[FormulaValue::Text("HELLO".into())]),
            FormulaValue::Text("hello".into())
        );
        assert_eq!(
            fn_trim(&[FormulaValue::Text("  hello  ".into())]),
            FormulaValue::Text("hello".into())
        );
    }

    #[test]
    fn if_true_false() {
        assert_eq!(
            fn_if(&[
                FormulaValue::Bool(true),
                FormulaValue::Number(1.0),
                FormulaValue::Number(0.0)
            ]),
            FormulaValue::Number(1.0)
        );
        assert_eq!(
            fn_if(&[
                FormulaValue::Bool(false),
                FormulaValue::Number(1.0),
                FormulaValue::Number(0.0)
            ]),
            FormulaValue::Number(0.0)
        );
    }

    #[test]
    fn if_numeric_condition() {
        assert_eq!(
            fn_if(&[
                FormulaValue::Number(5.0),
                FormulaValue::Text("yes".into()),
                FormulaValue::Text("no".into())
            ]),
            FormulaValue::Text("yes".into())
        );
        assert_eq!(
            fn_if(&[
                FormulaValue::Number(0.0),
                FormulaValue::Text("yes".into()),
                FormulaValue::Text("no".into())
            ]),
            FormulaValue::Text("no".into())
        );
    }

    #[test]
    fn and_or_not() {
        assert_eq!(
            fn_and(&[FormulaValue::Bool(true), FormulaValue::Bool(true)]),
            FormulaValue::Bool(true)
        );
        assert_eq!(
            fn_and(&[FormulaValue::Bool(true), FormulaValue::Bool(false)]),
            FormulaValue::Bool(false)
        );
        assert_eq!(
            fn_or(&[FormulaValue::Bool(false), FormulaValue::Bool(true)]),
            FormulaValue::Bool(true)
        );
        assert_eq!(
            fn_or(&[FormulaValue::Bool(false), FormulaValue::Bool(false)]),
            FormulaValue::Bool(false)
        );
        assert_eq!(
            fn_not(&[FormulaValue::Bool(true)]),
            FormulaValue::Bool(false)
        );
    }

    #[test]
    fn countif_basic() {
        let result = fn_countif(&[
            FormulaValue::Number(1.0),
            FormulaValue::Number(2.0),
            FormulaValue::Number(1.0),
            FormulaValue::Number(3.0),
            FormulaValue::Number(1.0), // criteria
        ]);
        assert_eq!(result, FormulaValue::Number(2.0));
    }

    #[test]
    fn sumif_basic() {
        let result = fn_sumif(&[
            FormulaValue::Number(1.0),
            FormulaValue::Number(2.0),
            FormulaValue::Number(1.0),
            FormulaValue::Number(3.0),
            FormulaValue::Number(1.0), // criteria
        ]);
        assert_eq!(result, FormulaValue::Number(2.0));
    }

    #[test]
    fn countif_text() {
        let result = fn_countif(&[
            FormulaValue::Text("apple".into()),
            FormulaValue::Text("banana".into()),
            FormulaValue::Text("apple".into()),
            FormulaValue::Text("Apple".into()), // criteria (case-insensitive)
        ]);
        assert_eq!(result, FormulaValue::Number(2.0));
    }

    #[test]
    fn registry_with_defaults() {
        let reg = FunctionRegistry::with_defaults();
        assert!(reg.has("SUM"));
        assert!(reg.has("IF"));
        assert!(reg.has("CONCAT"));
        assert!(!reg.has("VLOOKUP")); // not implemented yet
    }

    #[test]
    fn registry_custom_function() {
        let mut reg = FunctionRegistry::new();
        assert!(!reg.has("CUSTOM"));

        reg.register(
            "CUSTOM",
            |_args| FormulaValue::Number(42.0),
            Some(0),
        );
        assert!(reg.has("CUSTOM"));

        let (func, count) = reg.get("CUSTOM").unwrap();
        assert_eq!(*count, Some(0));
        assert_eq!(func(&[]), FormulaValue::Number(42.0));
    }

    #[test]
    fn registry_case_insensitive() {
        let reg = FunctionRegistry::with_defaults();
        assert!(reg.has("sum"));
        assert!(reg.has("Sum"));
        assert!(reg.has("SUM"));
    }
}
