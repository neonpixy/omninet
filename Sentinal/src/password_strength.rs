//! Password strength estimation for Omnidea.
//!
//! Provides entropy-based strength estimation with penalties for common
//! patterns, sequential characters, and repeated sequences. Crack time
//! is calculated assuming 10 GPUs against PBKDF2 at 600K iterations
//! (~300,000 guesses/sec).

use serde::{Deserialize, Serialize};

// --- Constants ---

/// Assumed guesses per second: 10 GPUs against PBKDF2-HMAC-SHA256 at 600K iterations.
const GUESSES_PER_SECOND: f64 = 300_000.0;

/// Entropy threshold for each strength tier (in bits).
const WEAK_THRESHOLD: f64 = 30.0;
const FAIR_THRESHOLD: f64 = 50.0;
const STRONG_THRESHOLD: f64 = 80.0;

/// Penalty multiplier when all characters are the same.
const ALL_SAME_PENALTY: f64 = 0.1;

/// Penalty per sequential run detected (subtracted from entropy).
const SEQUENTIAL_PENALTY: f64 = 5.0;

/// Penalty when a repeated pattern is detected (multiplier on entropy).
const REPEAT_PATTERN_PENALTY: f64 = 0.6;

// --- Public types ---

/// Estimated strength of a password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordStrength {
    /// Estimated entropy in bits.
    pub entropy_bits: f64,
    /// Strength tier.
    pub tier: StrengthTier,
    /// Human-readable hint for improving the password.
    pub hint: String,
    /// Estimated crack time description (e.g., "centuries", "3 hours").
    pub crack_time: String,
}

/// Password strength classification, ordered from weakest to strongest.
///
/// The tier boundaries are based on entropy bits: Weak (< 30), Fair (30-49),
/// Strong (50-79), Excellent (80+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StrengthTier {
    /// Under 30 bits of entropy -- trivially crackable. Common passwords,
    /// short strings, all-same-character, and keyboard walks land here.
    Weak,
    /// 30-49 bits of entropy -- resists casual attacks but not a determined
    /// adversary. Acceptable only for low-value accounts.
    Fair,
    /// 50-79 bits of entropy -- solid for most purposes. Would take years
    /// to crack with 10 GPUs against PBKDF2 at 600K iterations.
    Strong,
    /// 80+ bits of entropy -- centuries or more to crack. Long passphrases
    /// and high-entropy random strings reach this tier.
    Excellent,
}

// --- Public API ---

/// Estimate the strength of a password.
///
/// Calculates entropy from character class diversity, applies penalties
/// for common patterns, and computes a crack time estimate against
/// PBKDF2 at 600K iterations with 10 GPUs.
pub fn estimate_strength(password: &str) -> PasswordStrength {
    // Empty or common passwords are always Weak.
    if password.is_empty() {
        return PasswordStrength {
            entropy_bits: 0.0,
            tier: StrengthTier::Weak,
            hint: "Enter a password".to_string(),
            crack_time: "instant".to_string(),
        };
    }

    if is_common_password(password) {
        return PasswordStrength {
            entropy_bits: 0.0,
            tier: StrengthTier::Weak,
            hint: "This is a very common password — pick something unique".to_string(),
            crack_time: "instant".to_string(),
        };
    }

    // Calculate base entropy from character classes.
    let pool_size = character_pool_size(password);
    let char_count = password.chars().count();
    let mut entropy = char_count as f64 * (pool_size as f64).log2();

    // Apply penalties.
    if all_same_char(password) {
        entropy *= ALL_SAME_PENALTY;
    }

    let sequential_count = count_sequential_runs(password);
    entropy -= sequential_count as f64 * SEQUENTIAL_PENALTY;

    if has_repeated_pattern(password) {
        entropy *= REPEAT_PATTERN_PENALTY;
    }

    // Clamp to zero.
    if entropy < 0.0 {
        entropy = 0.0;
    }

    let tier = tier_from_entropy(entropy);
    let crack_time = format_crack_time(entropy);
    let hint = generate_hint(password, tier);

    PasswordStrength {
        entropy_bits: entropy,
        tier,
        hint,
        crack_time,
    }
}

// --- Internals ---

/// Check if the password is in the common passwords list (case-insensitive).
fn is_common_password(password: &str) -> bool {
    let lower = password.to_lowercase();
    COMMON_PASSWORDS.iter().any(|&common| common == lower)
}

/// Calculate the character pool size based on which classes are present.
fn character_pool_size(password: &str) -> u32 {
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_symbol = false;
    let mut has_unicode = false;

    for ch in password.chars() {
        match ch {
            'a'..='z' => has_lower = true,
            'A'..='Z' => has_upper = true,
            '0'..='9' => has_digit = true,
            // ASCII printable symbols.
            '!'..='/' | ':'..='@' | '['..='`' | '{'..='~' | ' ' => has_symbol = true,
            _ => has_unicode = true,
        }
    }

    let mut pool: u32 = 0;
    if has_lower {
        pool += 26;
    }
    if has_upper {
        pool += 26;
    }
    if has_digit {
        pool += 10;
    }
    if has_symbol {
        pool += 32;
    }
    if has_unicode {
        pool += 128; // Conservative estimate for non-ASCII.
    }

    // Minimum pool of 1 to avoid log2(0).
    pool.max(1)
}

/// Returns true if every character in the string is the same.
fn all_same_char(password: &str) -> bool {
    let mut chars = password.chars();
    let Some(first) = chars.next() else {
        return true;
    };
    chars.all(|ch| ch == first)
}

/// Count sequential character runs (abc, 123, cba, 321, qwerty substrings).
fn count_sequential_runs(password: &str) -> usize {
    let chars: Vec<char> = password.chars().collect();
    if chars.len() < 3 {
        return 0;
    }

    let mut runs = 0;

    // Check ascending/descending character sequences.
    for window in chars.windows(3) {
        let a = window[0] as i32;
        let b = window[1] as i32;
        let c = window[2] as i32;

        // Ascending: a, b, c where each is +1.
        if b - a == 1 && c - b == 1 {
            runs += 1;
        }
        // Descending: c, b, a where each is -1.
        if a - b == 1 && b - c == 1 {
            runs += 1;
        }
    }

    // Check keyboard row sequences.
    let lower = password.to_lowercase();
    for pattern in KEYBOARD_SEQUENCES {
        if lower.contains(pattern) {
            runs += 1;
        }
    }

    runs
}

/// Check if the password contains a repeated pattern (e.g., "abcabc").
fn has_repeated_pattern(password: &str) -> bool {
    let chars: Vec<char> = password.chars().collect();
    let len = chars.len();
    if len < 4 {
        return false;
    }

    // Check if the string is built from a repeating unit of length 1..len/2.
    for unit_len in 1..=len / 2 {
        if len % unit_len != 0 {
            continue;
        }
        let unit = &chars[..unit_len];
        let repeats = len / unit_len;
        if repeats >= 2 && (0..repeats).all(|i| chars[i * unit_len..(i + 1) * unit_len] == *unit)
        {
            return true;
        }
    }

    false
}

/// Map entropy bits to a strength tier.
fn tier_from_entropy(entropy: f64) -> StrengthTier {
    if entropy < WEAK_THRESHOLD {
        StrengthTier::Weak
    } else if entropy < FAIR_THRESHOLD {
        StrengthTier::Fair
    } else if entropy < STRONG_THRESHOLD {
        StrengthTier::Strong
    } else {
        StrengthTier::Excellent
    }
}

/// Format crack time as a human-readable string.
fn format_crack_time(entropy: f64) -> String {
    if entropy <= 0.0 {
        return "instant".to_string();
    }

    // seconds = 2^entropy / guesses_per_second
    // Use log to avoid overflow for large entropy values.
    let log2_seconds = entropy - GUESSES_PER_SECOND.log2();

    if log2_seconds < 0.0 {
        return "instant".to_string();
    }

    // Convert to seconds if feasible, otherwise work in log space.
    // log2(60) ≈ 5.9, log2(3600) ≈ 11.8, log2(86400) ≈ 16.4
    // log2(365.25*86400) ≈ 24.9, log2(100*365.25*86400) ≈ 31.5
    // log2(1000*365.25*86400) ≈ 34.9

    const LOG2_MINUTE: f64 = 5.907;
    const LOG2_HOUR: f64 = 11.811;
    const LOG2_DAY: f64 = 16.400;
    const LOG2_YEAR: f64 = 24.919;
    const LOG2_CENTURY: f64 = 31.547;
    const LOG2_MILLENNIUM: f64 = 34.870;

    if log2_seconds < 1.0 {
        "seconds".to_string()
    } else if log2_seconds < LOG2_MINUTE {
        let secs = 2_f64.powf(log2_seconds);
        format!("{} seconds", secs.round() as u64)
    } else if log2_seconds < LOG2_HOUR {
        let mins = 2_f64.powf(log2_seconds - LOG2_MINUTE);
        format!("{} minutes", mins.round().max(1.0) as u64)
    } else if log2_seconds < LOG2_DAY {
        let hours = 2_f64.powf(log2_seconds - LOG2_HOUR);
        format!("{} hours", hours.round().max(1.0) as u64)
    } else if log2_seconds < LOG2_YEAR {
        let days = 2_f64.powf(log2_seconds - LOG2_DAY);
        format!("{} days", days.round().max(1.0) as u64)
    } else if log2_seconds < LOG2_CENTURY {
        let years = 2_f64.powf(log2_seconds - LOG2_YEAR);
        format!("{} years", years.round().max(1.0) as u64)
    } else if log2_seconds < LOG2_MILLENNIUM {
        "centuries".to_string()
    } else {
        "millennia".to_string()
    }
}

/// Generate a hint for improving the password.
fn generate_hint(password: &str, tier: StrengthTier) -> String {
    match tier {
        StrengthTier::Excellent => {
            return "Excellent — this would take centuries to crack".to_string();
        }
        StrengthTier::Strong => {
            return "Great password!".to_string();
        }
        _ => {}
    }

    let mut suggestions = Vec::new();

    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password.chars().any(|c| {
        c.is_ascii_punctuation() || c == ' '
    });
    let char_count = password.chars().count();

    if !has_upper {
        suggestions.push("Add uppercase letters");
    }
    if !has_digit {
        suggestions.push("Add numbers");
    }
    if !has_symbol {
        suggestions.push("Add symbols like !@#$%");
    }
    if char_count < 12 {
        suggestions.push("Make it longer — 12+ characters is much stronger");
    }

    suggestions.push(
        "Try a passphrase: several random words like 'correct horse battery staple'",
    );

    suggestions.join(". ")
}

/// Keyboard row sequences to check for.
const KEYBOARD_SEQUENCES: &[&str] = &[
    "qwerty",
    "qwertz",
    "azerty",
    "asdf",
    "zxcv",
    "qwer",
    "wasd",
    "1234",
    "2345",
    "3456",
    "4567",
    "5678",
    "6789",
    "7890",
];

/// Top ~200 most common passwords (lowercase for case-insensitive comparison).
const COMMON_PASSWORDS: &[&str] = &[
    "123456",
    "password",
    "12345678",
    "qwerty",
    "123456789",
    "12345",
    "1234",
    "111111",
    "1234567",
    "dragon",
    "123123",
    "baseball",
    "abc123",
    "football",
    "monkey",
    "letmein",
    "shadow",
    "master",
    "666666",
    "qwertyuiop",
    "123321",
    "mustang",
    "1234567890",
    "michael",
    "654321",
    "superman",
    "1qaz2wsx",
    "7777777",
    "121212",
    "000000",
    "qazwsx",
    "123qwe",
    "killer",
    "trustno1",
    "jordan",
    "jennifer",
    "zxcvbnm",
    "asdfgh",
    "hunter",
    "buster",
    "soccer",
    "harley",
    "batman",
    "andrew",
    "tigger",
    "sunshine",
    "iloveyou",
    "2000",
    "charlie",
    "robert",
    "thomas",
    "hockey",
    "ranger",
    "daniel",
    "starwars",
    "klaster",
    "112233",
    "george",
    "computer",
    "michelle",
    "jessica",
    "pepper",
    "1111",
    "zxcvbn",
    "555555",
    "11111111",
    "131313",
    "freedom",
    "777777",
    "pass",
    "maggie",
    "159753",
    "aaaaaa",
    "ginger",
    "princess",
    "joshua",
    "cheese",
    "amanda",
    "summer",
    "love",
    "ashley",
    "nicole",
    "chelsea",
    "biteme",
    "matthew",
    "access",
    "yankees",
    "987654321",
    "dallas",
    "austin",
    "thunder",
    "taylor",
    "matrix",
    "minecraft",
    "william",
    "corvette",
    "hello",
    "martin",
    "heather",
    "secret",
    "merlin",
    "diamond",
    "1234qwer",
    "gfhjkm",
    "hammer",
    "silver",
    "222222",
    "88888888",
    "anthony",
    "justin",
    "test",
    "bailey",
    "q1w2e3r4t5",
    "patrick",
    "internet",
    "scooter",
    "orange",
    "11111",
    "golfer",
    "cookie",
    "richard",
    "samantha",
    "bigdog",
    "guitar",
    "jackson",
    "whatever",
    "mickey",
    "chicken",
    "sparky",
    "snoopy",
    "maverick",
    "phoenix",
    "camaro",
    "peanut",
    "morgan",
    "welcome",
    "falcon",
    "cowboy",
    "ferrari",
    "samsung",
    "andrea",
    "smokey",
    "steelers",
    "joseph",
    "mercedes",
    "dakota",
    "arsenal",
    "eagles",
    "melissa",
    "boomer",
    "booboo",
    "spider",
    "nascar",
    "monster",
    "tigers",
    "yellow",
    "xxxxxx",
    "123123123",
    "gateway",
    "marina",
    "diablo",
    "bulldog",
    "qwer1234",
    "compaq",
    "purple",
    "hardcore",
    "banana",
    "junior",
    "hannah",
    "123654",
    "lazarus",
    "nicholas",
    "swimming",
    "andrea1",
    "trustme",
    "admin",
    "login",
    "password1",
    "password123",
    "changeme",
    "letmein1",
    "welcome1",
    "passw0rd",
    "p@ssword",
    "p@ssw0rd",
    "abc1234",
    "abcdef",
    "abcdefg",
    "google",
    "apple",
    "facebook",
    "linkedin",
    "twitter",
    "qwerty123",
    "admin123",
    "root",
    "toor",
    "pass123",
    "test123",
    "guest",
    "master123",
    "dragon1",
    "baseball1",
    "shadow1",
    "monkey1",
    "696969",
    "letmein!",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_password_is_weak() {
        let result = estimate_strength("");
        assert_eq!(result.tier, StrengthTier::Weak);
        assert_eq!(result.entropy_bits, 0.0);
        assert_eq!(result.crack_time, "instant");
        assert!(!result.hint.is_empty());
    }

    #[test]
    fn test_common_password_is_weak() {
        let result = estimate_strength("password");
        assert_eq!(result.tier, StrengthTier::Weak);
        assert_eq!(result.entropy_bits, 0.0);
        assert_eq!(result.crack_time, "instant");
    }

    #[test]
    fn test_common_password_case_insensitive() {
        let result = estimate_strength("PASSWORD");
        assert_eq!(result.tier, StrengthTier::Weak);

        let result = estimate_strength("Password");
        assert_eq!(result.tier, StrengthTier::Weak);
    }

    #[test]
    fn test_123456_is_weak() {
        let result = estimate_strength("123456");
        assert_eq!(result.tier, StrengthTier::Weak);
        assert_eq!(result.crack_time, "instant");
    }

    #[test]
    fn test_short_all_classes_is_fair_or_strong() {
        // 8 chars with all classes: aB3$xY7!
        let result = estimate_strength("aB3$xY7!");
        assert!(
            result.tier >= StrengthTier::Fair,
            "expected Fair or better, got {:?} with {:.1} bits",
            result.tier,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_passphrase_is_strong_or_excellent() {
        let result = estimate_strength("correct horse battery staple");
        assert!(
            result.tier >= StrengthTier::Strong,
            "expected Strong or better, got {:?} with {:.1} bits",
            result.tier,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_troubador_is_fair_or_strong() {
        let result = estimate_strength("Tr0ub4dor&3");
        assert!(
            result.tier >= StrengthTier::Fair,
            "expected Fair or better, got {:?} with {:.1} bits",
            result.tier,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_long_random_string_is_excellent() {
        let result = estimate_strength("kX9!mP2@nQ7#jL4$vR8%");
        assert_eq!(
            result.tier,
            StrengthTier::Excellent,
            "20-char random with all classes should be Excellent, got {:?} with {:.1} bits",
            result.tier,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_tier_ordering() {
        assert!(StrengthTier::Weak < StrengthTier::Fair);
        assert!(StrengthTier::Fair < StrengthTier::Strong);
        assert!(StrengthTier::Strong < StrengthTier::Excellent);
    }

    #[test]
    fn test_crack_time_is_nonempty() {
        let result = estimate_strength("anything");
        assert!(!result.crack_time.is_empty());
    }

    #[test]
    fn test_hint_nonempty_for_weak() {
        let result = estimate_strength("abc");
        assert_eq!(result.tier, StrengthTier::Weak);
        assert!(!result.hint.is_empty());
    }

    #[test]
    fn test_hint_for_strong_is_positive() {
        let result = estimate_strength("kX9!mP2@nQ7#jL4$vR8%");
        assert!(
            result.hint.contains("Excellent") || result.hint.contains("Great"),
            "strong/excellent hint should be positive: {}",
            result.hint,
        );
    }

    #[test]
    fn test_all_same_char_penalized() {
        let result = estimate_strength("aaaaaaaaaa");
        assert_eq!(
            result.tier,
            StrengthTier::Weak,
            "10x same char should be Weak, got {:?} with {:.1} bits",
            result.tier,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_sequential_penalized() {
        let result = estimate_strength("abcdefghij");
        // Should be penalized for sequential runs.
        assert!(
            result.entropy_bits < 47.0, // 10 * log2(26) ≈ 47, but sequentials should reduce it
            "sequential password should have reduced entropy, got {:.1}",
            result.entropy_bits,
        );
    }

    #[test]
    fn test_repeated_pattern_penalized() {
        let result = estimate_strength("abcabc");
        assert!(
            result.entropy_bits < 28.0, // 6 * log2(26) ≈ 28, but repeat should reduce it
            "repeated pattern should have reduced entropy, got {:.1}",
            result.entropy_bits,
        );
    }

    #[test]
    fn test_keyboard_sequence_penalized() {
        let result = estimate_strength("qwerty123");
        // Should be caught as common password.
        // If not, keyboard sequences should penalize it.
        assert!(
            result.tier <= StrengthTier::Fair,
            "keyboard sequence should be Weak or Fair, got {:?}",
            result.tier,
        );
    }

    #[test]
    fn test_unicode_password() {
        // Unicode characters should expand the pool significantly.
        let result = estimate_strength("Пароль123!日本語");
        assert!(
            result.entropy_bits > 30.0,
            "unicode password should have decent entropy, got {:.1}",
            result.entropy_bits,
        );
    }

    #[test]
    fn test_serialization_round_trip() {
        let result = estimate_strength("test-password-123!");
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PasswordStrength = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tier, result.tier);
        assert_eq!(parsed.hint, result.hint);
        assert_eq!(parsed.crack_time, result.crack_time);
        assert!(
            (parsed.entropy_bits - result.entropy_bits).abs() < 1e-10,
            "entropy should survive JSON round-trip: {} vs {}",
            parsed.entropy_bits,
            result.entropy_bits,
        );
    }

    #[test]
    fn test_strength_increases_with_length() {
        let short = estimate_strength("aB3$");
        let medium = estimate_strength("aB3$xY7!");
        let long = estimate_strength("aB3$xY7!kM2@nQ5#");

        assert!(
            medium.entropy_bits > short.entropy_bits,
            "longer password should have more entropy",
        );
        assert!(
            long.entropy_bits > medium.entropy_bits,
            "even longer password should have more entropy",
        );
    }

    #[test]
    fn test_more_character_classes_increase_entropy() {
        // Verify that adding character classes increases the pool size.
        let pool_lower = character_pool_size("abcdefghij");
        let pool_mixed = character_pool_size("aBcDeFgHiJ");
        assert!(pool_mixed > pool_lower);

        // And the non-sequential version should clearly show it.
        let lower_rand = estimate_strength("jqxvbfmthk");
        let mixed_rand = estimate_strength("jQxVbFmThK");
        assert!(mixed_rand.entropy_bits > lower_rand.entropy_bits);
    }

    #[test]
    fn test_format_crack_time_edge_cases() {
        assert_eq!(format_crack_time(0.0), "instant");
        assert_eq!(format_crack_time(-5.0), "instant");
        // Very high entropy should say millennia.
        assert_eq!(format_crack_time(200.0), "millennia");
    }

    #[test]
    fn test_common_passwords_list_has_entries() {
        assert!(COMMON_PASSWORDS.len() >= 190, "should have ~200 common passwords");
    }

    #[test]
    fn test_single_char_password() {
        let result = estimate_strength("a");
        assert_eq!(result.tier, StrengthTier::Weak);
    }

    #[test]
    fn test_spaces_count_as_symbols() {
        let pool_no_space = character_pool_size("abcdef");
        let pool_with_space = character_pool_size("abc def");
        assert!(
            pool_with_space > pool_no_space,
            "space should expand the character pool",
        );
    }
}
