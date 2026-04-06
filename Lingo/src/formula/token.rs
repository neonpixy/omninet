use super::error::{FormulaError, Span};

/// A single token produced by the formula tokenizer.
#[derive(Clone, Debug, PartialEq)]
pub enum FormulaToken {
    /// A numeric literal (e.g., 42, 3.14, 1e10).
    Number(f64),
    /// A string literal (e.g., "hello").
    Text(String),
    /// A boolean literal (TRUE or FALSE).
    Bool(bool),
    /// A cell reference (e.g., "A1", "$A$1", "Sheet1!B2").
    CellRef(String),
    /// Range separator `:`.
    Colon,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `^`
    Caret,
    /// `=`
    Equal,
    /// `<>`
    NotEqual,
    /// `<`
    LessThan,
    /// `<=`
    LessEqual,
    /// `>`
    GreaterThan,
    /// `>=`
    GreaterEqual,
    /// `&` (string concatenation)
    Ampersand,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,` (argument separator)
    Comma,
    /// A function name followed by `(`. Always uppercase canonical.
    FunctionName(String),
    /// End of input.
    Eof,
}

/// Tokenizer for spreadsheet formulas.
pub struct FormulaTokenizer {
    source: Vec<char>,
    position: usize,
}

impl FormulaTokenizer {
    /// Create a new tokenizer for the given input.
    pub fn new(input: &str) -> Self {
        Self {
            source: input.chars().collect(),
            position: 0,
        }
    }

    /// Tokenize the entire input, returning tokens with their spans.
    pub fn tokenize(&mut self) -> Result<Vec<(FormulaToken, Span)>, FormulaError> {
        // Skip leading `=` if present (formulas start with =).
        if self.peek() == Some('=') {
            self.advance();
        }

        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();

            if self.is_at_end() {
                tokens.push((FormulaToken::Eof, Span::new(self.position, self.position)));
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<(FormulaToken, Span), FormulaError> {
        let start = self.position;
        let ch = self.peek().expect("next_token called only when not at end");

        match ch {
            // String literals
            '"' => self.read_string(),

            // Numbers
            '0'..='9' => self.read_number(),

            // Dot starting a number (e.g., .5)
            '.' if self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) => self.read_number(),

            // Operators and punctuation
            '+' => {
                self.advance();
                Ok((FormulaToken::Plus, Span::new(start, self.position)))
            }
            '-' => {
                self.advance();
                Ok((FormulaToken::Minus, Span::new(start, self.position)))
            }
            '*' => {
                self.advance();
                Ok((FormulaToken::Star, Span::new(start, self.position)))
            }
            '/' => {
                self.advance();
                Ok((FormulaToken::Slash, Span::new(start, self.position)))
            }
            '%' => {
                self.advance();
                Ok((FormulaToken::Percent, Span::new(start, self.position)))
            }
            '^' => {
                self.advance();
                Ok((FormulaToken::Caret, Span::new(start, self.position)))
            }
            '&' => {
                self.advance();
                Ok((FormulaToken::Ampersand, Span::new(start, self.position)))
            }
            '(' => {
                self.advance();
                Ok((FormulaToken::LParen, Span::new(start, self.position)))
            }
            ')' => {
                self.advance();
                Ok((FormulaToken::RParen, Span::new(start, self.position)))
            }
            ',' => {
                self.advance();
                Ok((FormulaToken::Comma, Span::new(start, self.position)))
            }
            ':' => {
                self.advance();
                Ok((FormulaToken::Colon, Span::new(start, self.position)))
            }
            '=' => {
                self.advance();
                Ok((FormulaToken::Equal, Span::new(start, self.position)))
            }
            '<' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    Ok((FormulaToken::NotEqual, Span::new(start, self.position)))
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok((FormulaToken::LessEqual, Span::new(start, self.position)))
                } else {
                    Ok((FormulaToken::LessThan, Span::new(start, self.position)))
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok((FormulaToken::GreaterEqual, Span::new(start, self.position)))
                } else {
                    Ok((FormulaToken::GreaterThan, Span::new(start, self.position)))
                }
            }

            // $ can start an absolute cell reference
            '$' => self.read_cell_ref_or_identifier(),

            // Letters — could be a cell ref, function name, or boolean
            c if c.is_ascii_alphabetic() => self.read_cell_ref_or_identifier(),

            _ => Err(FormulaError::UnexpectedToken {
                span: Span::new(start, start + 1),
                message: format!("unexpected character: '{}'", ch),
            }),
        }
    }

    fn read_string(&mut self) -> Result<(FormulaToken, Span), FormulaError> {
        let start = self.position;
        self.advance(); // skip opening "

        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(FormulaError::UnterminatedString {
                        span: Span::new(start, self.position),
                    });
                }
                Some('"') => {
                    self.advance();
                    // Excel-style escaped quotes: "" inside a string means literal "
                    if self.peek() == Some('"') {
                        value.push('"');
                        self.advance();
                    } else {
                        break;
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }

        Ok((FormulaToken::Text(value), Span::new(start, self.position)))
    }

    fn read_number(&mut self) -> Result<(FormulaToken, Span), FormulaError> {
        let start = self.position;
        let mut num_str = String::new();

        // Integer part
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                self.advance();
            } else {
                break;
            }
        }

        // Decimal part
        if self.peek() == Some('.') {
            num_str.push('.');
            self.advance();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Exponent part
        if let Some(c) = self.peek() {
            if c == 'e' || c == 'E' {
                num_str.push(c);
                self.advance();
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        num_str.push(sign);
                        self.advance();
                    }
                }
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        num_str.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        let value: f64 = num_str.parse().map_err(|_| FormulaError::UnexpectedToken {
            span: Span::new(start, self.position),
            message: format!("invalid number: {}", num_str),
        })?;

        Ok((FormulaToken::Number(value), Span::new(start, self.position)))
    }

    fn read_cell_ref_or_identifier(&mut self) -> Result<(FormulaToken, Span), FormulaError> {
        let start = self.position;

        // Collect the full identifier (including $ for absolute refs and ! for sheet refs).
        // We need to figure out if this is:
        //   1. A boolean literal (TRUE, FALSE)
        //   2. A function name (followed by open paren)
        //   3. A cell reference (letters + digits, with optional $ and Sheet!)
        let mut raw = String::new();

        // Handle optional sheet prefix: "SheetName!" or letters before "!"
        // First, collect everything that could be part of a cell ref or identifier.
        let mut has_exclamation = false;

        // Collect the full token text
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_alphanumeric() || c == '_' => {
                    raw.push(c);
                    self.advance();
                }
                Some('$') => {
                    raw.push('$');
                    self.advance();
                }
                Some('!') if !has_exclamation => {
                    raw.push('!');
                    self.advance();
                    has_exclamation = true;
                }
                _ => break,
            }
        }

        let upper = raw.to_uppercase();

        // Check for boolean literals
        if upper == "TRUE" {
            return Ok((FormulaToken::Bool(true), Span::new(start, self.position)));
        }
        if upper == "FALSE" {
            return Ok((FormulaToken::Bool(false), Span::new(start, self.position)));
        }

        // Check if it's a function name (followed by opening paren)
        self.skip_whitespace();
        if self.peek() == Some('(') {
            return Ok((
                FormulaToken::FunctionName(upper),
                Span::new(start, self.position),
            ));
        }

        // Otherwise it's a cell reference
        // Validate that it looks like a cell reference
        if is_cell_ref(&raw) {
            Ok((FormulaToken::CellRef(raw), Span::new(start, self.position)))
        } else {
            // Could be an identifier we don't understand — treat as a cell ref
            // and let the parser/evaluator decide
            Ok((FormulaToken::CellRef(raw), Span::new(start, self.position)))
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.position).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.source.get(self.position + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.position).copied();
        self.position += 1;
        ch
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.source.len()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }
}

/// Basic heuristic check for whether a string looks like a cell reference.
/// This is intentionally permissive — the parser will do deeper validation.
fn is_cell_ref(s: &str) -> bool {
    // Strip optional sheet prefix
    let after_sheet = if let Some(idx) = s.find('!') {
        &s[idx + 1..]
    } else {
        s
    };

    // After sheet prefix, should be [$]letters[$]digits
    let mut chars = after_sheet.chars().peekable();

    // Skip optional $
    if chars.peek() == Some(&'$') {
        chars.next();
    }

    // Must have at least one letter
    let mut has_letter = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            has_letter = true;
            chars.next();
        } else {
            break;
        }
    }

    if !has_letter {
        return false;
    }

    // Skip optional $
    if chars.peek() == Some(&'$') {
        chars.next();
    }

    // Must have at least one digit
    let mut has_digit = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            has_digit = true;
            chars.next();
        } else {
            break;
        }
    }

    // Must have consumed everything
    has_digit && chars.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<FormulaToken> {
        let mut tokenizer = FormulaTokenizer::new(input);
        tokenizer
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    #[test]
    fn simple_number() {
        let tokens = tokenize("=42");
        assert_eq!(tokens, vec![FormulaToken::Number(42.0), FormulaToken::Eof]);
    }

    #[test]
    fn decimal_number() {
        let tokens = tokenize("=3.15");
        assert_eq!(
            tokens,
            vec![FormulaToken::Number(3.15), FormulaToken::Eof]
        );
    }

    #[test]
    fn scientific_notation() {
        let tokens = tokenize("=1e10");
        assert_eq!(
            tokens,
            vec![FormulaToken::Number(1e10), FormulaToken::Eof]
        );

        let tokens = tokenize("=2.5E-3");
        assert_eq!(
            tokens,
            vec![FormulaToken::Number(2.5e-3), FormulaToken::Eof]
        );
    }

    #[test]
    fn dot_leading_number() {
        let tokens = tokenize("=.5");
        assert_eq!(tokens, vec![FormulaToken::Number(0.5), FormulaToken::Eof]);
    }

    #[test]
    fn string_literal() {
        let tokens = tokenize(r#"="hello""#);
        assert_eq!(
            tokens,
            vec![FormulaToken::Text("hello".into()), FormulaToken::Eof]
        );
    }

    #[test]
    fn string_with_escaped_quotes() {
        let tokens = tokenize(r#"="say ""hi""!""#);
        assert_eq!(
            tokens,
            vec![
                FormulaToken::Text(r#"say "hi"!"#.into()),
                FormulaToken::Eof
            ]
        );
    }

    #[test]
    fn unterminated_string() {
        let mut tokenizer = FormulaTokenizer::new(r#"="hello"#);
        assert!(tokenizer.tokenize().is_err());
    }

    #[test]
    fn boolean_literals() {
        let tokens = tokenize("=TRUE");
        assert_eq!(tokens, vec![FormulaToken::Bool(true), FormulaToken::Eof]);

        let tokens = tokenize("=false");
        assert_eq!(tokens, vec![FormulaToken::Bool(false), FormulaToken::Eof]);
    }

    #[test]
    fn cell_references() {
        let tokens = tokenize("=A1");
        assert_eq!(
            tokens,
            vec![FormulaToken::CellRef("A1".into()), FormulaToken::Eof]
        );

        let tokens = tokenize("=$A$1");
        assert_eq!(
            tokens,
            vec![FormulaToken::CellRef("$A$1".into()), FormulaToken::Eof]
        );

        let tokens = tokenize("=AB123");
        assert_eq!(
            tokens,
            vec![FormulaToken::CellRef("AB123".into()), FormulaToken::Eof]
        );
    }

    #[test]
    fn cell_ref_with_sheet() {
        let tokens = tokenize("=Sheet1!B2");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("Sheet1!B2".into()),
                FormulaToken::Eof
            ]
        );
    }

    #[test]
    fn function_name() {
        let tokens = tokenize("=SUM(A1)");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::FunctionName("SUM".into()),
                FormulaToken::LParen,
                FormulaToken::CellRef("A1".into()),
                FormulaToken::RParen,
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn function_name_case_insensitive() {
        let tokens = tokenize("=sum(1)");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::FunctionName("SUM".into()),
                FormulaToken::LParen,
                FormulaToken::Number(1.0),
                FormulaToken::RParen,
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn operators() {
        let tokens = tokenize("=1+2-3*4/5^6%");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::Number(1.0),
                FormulaToken::Plus,
                FormulaToken::Number(2.0),
                FormulaToken::Minus,
                FormulaToken::Number(3.0),
                FormulaToken::Star,
                FormulaToken::Number(4.0),
                FormulaToken::Slash,
                FormulaToken::Number(5.0),
                FormulaToken::Caret,
                FormulaToken::Number(6.0),
                FormulaToken::Percent,
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn comparison_operators() {
        let tokens = tokenize("=A1=B1");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("A1".into()),
                FormulaToken::Equal,
                FormulaToken::CellRef("B1".into()),
                FormulaToken::Eof,
            ]
        );

        let tokens = tokenize("=A1<>B1");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("A1".into()),
                FormulaToken::NotEqual,
                FormulaToken::CellRef("B1".into()),
                FormulaToken::Eof,
            ]
        );

        let tokens = tokenize("=A1<=B1");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("A1".into()),
                FormulaToken::LessEqual,
                FormulaToken::CellRef("B1".into()),
                FormulaToken::Eof,
            ]
        );

        let tokens = tokenize("=A1>=B1");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("A1".into()),
                FormulaToken::GreaterEqual,
                FormulaToken::CellRef("B1".into()),
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn ampersand_concat() {
        let tokens = tokenize(r#"="hello"&"world""#);
        assert_eq!(
            tokens,
            vec![
                FormulaToken::Text("hello".into()),
                FormulaToken::Ampersand,
                FormulaToken::Text("world".into()),
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn range_with_colon() {
        let tokens = tokenize("=A1:B5");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::CellRef("A1".into()),
                FormulaToken::Colon,
                FormulaToken::CellRef("B5".into()),
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn whitespace_handling() {
        let tokens = tokenize("= 1 + 2 ");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::Number(1.0),
                FormulaToken::Plus,
                FormulaToken::Number(2.0),
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn complex_formula() {
        let tokens = tokenize("=IF(A1>0, SUM(B1:B10), 0)");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::FunctionName("IF".into()),
                FormulaToken::LParen,
                FormulaToken::CellRef("A1".into()),
                FormulaToken::GreaterThan,
                FormulaToken::Number(0.0),
                FormulaToken::Comma,
                FormulaToken::FunctionName("SUM".into()),
                FormulaToken::LParen,
                FormulaToken::CellRef("B1".into()),
                FormulaToken::Colon,
                FormulaToken::CellRef("B10".into()),
                FormulaToken::RParen,
                FormulaToken::Comma,
                FormulaToken::Number(0.0),
                FormulaToken::RParen,
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn no_leading_equals() {
        // Should also work without leading =
        let tokens = tokenize("1+2");
        assert_eq!(
            tokens,
            vec![
                FormulaToken::Number(1.0),
                FormulaToken::Plus,
                FormulaToken::Number(2.0),
                FormulaToken::Eof,
            ]
        );
    }

    #[test]
    fn is_cell_ref_heuristic() {
        assert!(is_cell_ref("A1"));
        assert!(is_cell_ref("$A$1"));
        assert!(is_cell_ref("AB123"));
        assert!(is_cell_ref("Sheet1!A1"));
        assert!(is_cell_ref("$A1"));
        assert!(is_cell_ref("A$1"));
        assert!(!is_cell_ref("123"));
        assert!(!is_cell_ref("ABC"));
        assert!(!is_cell_ref("$"));
    }
}
