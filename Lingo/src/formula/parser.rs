use super::ast::{BinaryOp, FormulaCellRef, FormulaNode, UnaryOp};
use super::error::{FormulaError, Span};
use super::token::{FormulaToken, FormulaTokenizer};
use super::value::FormulaValue;

/// Recursive descent parser for spreadsheet formulas with Pratt-style
/// operator precedence.
///
/// Precedence (lowest to highest):
/// 1. Comparison (=, <>, <, <=, >, >=)
/// 2. Concatenation (&)
/// 3. Addition/Subtraction (+, -)
/// 4. Multiplication/Division (*, /)
/// 5. Exponentiation (^)
/// 6. Unary (-, %)
/// 7. Primary (literals, cell refs, function calls, parenthesized)
pub struct FormulaParser {
    tokens: Vec<(FormulaToken, Span)>,
    position: usize,
}

impl FormulaParser {
    /// Parse a formula string into an AST.
    pub fn parse(input: &str) -> Result<FormulaNode, FormulaError> {
        let mut tokenizer = FormulaTokenizer::new(input);
        let tokens = tokenizer.tokenize()?;

        let mut parser = Self {
            tokens,
            position: 0,
        };

        let node = parser.parse_expression()?;

        // Ensure we consumed everything (except Eof)
        if !parser.is_at_end() {
            let (token, span) = parser.current();
            return Err(FormulaError::UnexpectedToken {
                span: span.clone(),
                message: format!("unexpected token after expression: {:?}", token),
            });
        }

        Ok(node)
    }

    /// Parse an expression at the lowest precedence level.
    fn parse_expression(&mut self) -> Result<FormulaNode, FormulaError> {
        self.parse_comparison()
    }

    /// Precedence level 1: Comparison operators (=, <>, <, <=, >, >=).
    fn parse_comparison(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut left = self.parse_concatenation()?;

        loop {
            let op = match self.peek_token() {
                FormulaToken::Equal => BinaryOp::Eq,
                FormulaToken::NotEqual => BinaryOp::Ne,
                FormulaToken::LessThan => BinaryOp::Lt,
                FormulaToken::LessEqual => BinaryOp::Le,
                FormulaToken::GreaterThan => BinaryOp::Gt,
                FormulaToken::GreaterEqual => BinaryOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_concatenation()?;
            left = FormulaNode::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Precedence level 2: Concatenation (&).
    fn parse_concatenation(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut left = self.parse_addition()?;

        while self.peek_token() == FormulaToken::Ampersand {
            self.advance();
            let right = self.parse_addition()?;
            left = FormulaNode::BinaryOp {
                op: BinaryOp::Concat,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Precedence level 3: Addition and subtraction (+, -).
    fn parse_addition(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut left = self.parse_multiplication()?;

        loop {
            let op = match self.peek_token() {
                FormulaToken::Plus => BinaryOp::Add,
                FormulaToken::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = FormulaNode::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Precedence level 4: Multiplication and division (*, /).
    fn parse_multiplication(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut left = self.parse_exponentiation()?;

        loop {
            let op = match self.peek_token() {
                FormulaToken::Star => BinaryOp::Mul,
                FormulaToken::Slash => BinaryOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_exponentiation()?;
            left = FormulaNode::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Precedence level 5: Exponentiation (^). Right-associative.
    fn parse_exponentiation(&mut self) -> Result<FormulaNode, FormulaError> {
        let left = self.parse_unary()?;

        if self.peek_token() == FormulaToken::Caret {
            self.advance();
            // Right-associative: recurse into exponentiation again.
            let right = self.parse_exponentiation()?;
            Ok(FormulaNode::BinaryOp {
                op: BinaryOp::Pow,
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    /// Precedence level 6: Unary operators (-, postfix %).
    fn parse_unary(&mut self) -> Result<FormulaNode, FormulaError> {
        // Prefix unary minus
        if self.peek_token() == FormulaToken::Minus {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(FormulaNode::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            });
        }

        // Prefix unary plus (just ignore it)
        if self.peek_token() == FormulaToken::Plus {
            self.advance();
            return self.parse_unary();
        }

        let mut node = self.parse_primary()?;

        // Postfix percent
        while self.peek_token() == FormulaToken::Percent {
            self.advance();
            node = FormulaNode::UnaryOp {
                op: UnaryOp::Percent,
                operand: Box::new(node),
            };
        }

        Ok(node)
    }

    /// Precedence level 7: Primary expressions.
    fn parse_primary(&mut self) -> Result<FormulaNode, FormulaError> {
        let (token, span) = self.current();
        let token = token.clone();
        let span = span.clone();

        match token {
            FormulaToken::Number(n) => {
                self.advance();
                Ok(FormulaNode::Literal(FormulaValue::Number(n)))
            }
            FormulaToken::Text(s) => {
                self.advance();
                Ok(FormulaNode::Literal(FormulaValue::Text(s)))
            }
            FormulaToken::Bool(b) => {
                self.advance();
                Ok(FormulaNode::Literal(FormulaValue::Bool(b)))
            }
            FormulaToken::CellRef(ref_str) => {
                self.advance();
                let cell_ref = parse_cell_ref(&ref_str)?;

                // Check for range (A1:B5)
                if self.peek_token() == FormulaToken::Colon {
                    self.advance();
                    if let FormulaToken::CellRef(end_str) = self.peek_token().clone() {
                        self.advance();
                        let end_ref = parse_cell_ref(&end_str)?;
                        Ok(FormulaNode::Range {
                            start: cell_ref,
                            end: end_ref,
                        })
                    } else {
                        Err(FormulaError::UnexpectedToken {
                            span: self.current_span(),
                            message: "expected cell reference after ':'".into(),
                        })
                    }
                } else {
                    Ok(FormulaNode::CellRef(cell_ref))
                }
            }
            FormulaToken::FunctionName(name) => {
                self.advance();
                self.parse_function_call(&name)
            }
            FormulaToken::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect_token(&FormulaToken::RParen)?;
                Ok(FormulaNode::Parenthesized(Box::new(expr)))
            }
            FormulaToken::Eof => Err(FormulaError::UnexpectedToken {
                span,
                message: "unexpected end of formula".into(),
            }),
            _ => Err(FormulaError::UnexpectedToken {
                span,
                message: format!("unexpected token: {:?}", token),
            }),
        }
    }

    /// Parse a function call: name has already been consumed, expect ( args ).
    fn parse_function_call(&mut self, name: &str) -> Result<FormulaNode, FormulaError> {
        self.expect_token(&FormulaToken::LParen)?;

        let mut args = Vec::new();

        // Empty argument list
        if self.peek_token() == FormulaToken::RParen {
            self.advance();
            return Ok(FormulaNode::FunctionCall {
                name: name.to_string(),
                args,
            });
        }

        // First argument
        args.push(self.parse_expression()?);

        // Remaining arguments
        while self.peek_token() == FormulaToken::Comma {
            self.advance();
            args.push(self.parse_expression()?);
        }

        self.expect_token(&FormulaToken::RParen)?;

        Ok(FormulaNode::FunctionCall {
            name: name.to_string(),
            args,
        })
    }

    // --- Helpers ---

    fn peek_token(&self) -> FormulaToken {
        self.tokens
            .get(self.position)
            .map(|(t, _)| t.clone())
            .unwrap_or(FormulaToken::Eof)
    }

    fn current(&self) -> (FormulaToken, Span) {
        self.tokens
            .get(self.position)
            .cloned()
            .unwrap_or((FormulaToken::Eof, Span::new(0, 0)))
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.position)
            .map(|(_, s)| s.clone())
            .unwrap_or(Span::new(0, 0))
    }

    fn advance(&mut self) {
        if self.position < self.tokens.len() {
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_token(), FormulaToken::Eof)
    }

    fn expect_token(&mut self, expected: &FormulaToken) -> Result<(), FormulaError> {
        let (token, span) = self.current();
        if std::mem::discriminant(&token) == std::mem::discriminant(expected) {
            self.advance();
            Ok(())
        } else {
            Err(FormulaError::UnexpectedToken {
                span,
                message: format!("expected {:?}, got {:?}", expected, token),
            })
        }
    }
}

/// Parse a cell reference string (e.g., "A1", "$A$1", "Sheet1!$B$2") into a FormulaCellRef.
pub(crate) fn parse_cell_ref(s: &str) -> Result<FormulaCellRef, FormulaError> {
    let (sheet, remainder) = if let Some(idx) = s.find('!') {
        (Some(s[..idx].to_string()), &s[idx + 1..])
    } else {
        (None, s)
    };

    let mut chars = remainder.chars().peekable();

    // Optional $ for absolute column
    let abs_column = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    // Column letters
    let mut column = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            column.push(c.to_ascii_uppercase());
            chars.next();
        } else {
            break;
        }
    }

    if column.is_empty() {
        return Err(FormulaError::InvalidCellRef {
            reference: s.to_string(),
        });
    }

    // Optional $ for absolute row
    let abs_row = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    // Row digits
    let mut row_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            row_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if row_str.is_empty() {
        return Err(FormulaError::InvalidCellRef {
            reference: s.to_string(),
        });
    }

    let row: u32 = row_str.parse().map_err(|_| FormulaError::InvalidCellRef {
        reference: s.to_string(),
    })?;

    if row == 0 {
        return Err(FormulaError::InvalidCellRef {
            reference: s.to_string(),
        });
    }

    Ok(FormulaCellRef {
        sheet,
        column,
        row,
        abs_column,
        abs_row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_number() {
        let node = FormulaParser::parse("=42").unwrap();
        assert_eq!(node, FormulaNode::Literal(FormulaValue::Number(42.0)));
    }

    #[test]
    fn parse_simple_string() {
        let node = FormulaParser::parse(r#"="hello""#).unwrap();
        assert_eq!(
            node,
            FormulaNode::Literal(FormulaValue::Text("hello".into()))
        );
    }

    #[test]
    fn parse_bool() {
        let node = FormulaParser::parse("=TRUE").unwrap();
        assert_eq!(node, FormulaNode::Literal(FormulaValue::Bool(true)));
    }

    #[test]
    fn parse_cell_ref_simple() {
        let node = FormulaParser::parse("=A1").unwrap();
        match node {
            FormulaNode::CellRef(r) => {
                assert_eq!(r.column, "A");
                assert_eq!(r.row, 1);
                assert!(!r.abs_column);
                assert!(!r.abs_row);
                assert!(r.sheet.is_none());
            }
            _ => panic!("expected CellRef"),
        }
    }

    #[test]
    fn parse_absolute_cell_ref() {
        let node = FormulaParser::parse("=$A$1").unwrap();
        match node {
            FormulaNode::CellRef(r) => {
                assert!(r.abs_column);
                assert!(r.abs_row);
            }
            _ => panic!("expected CellRef"),
        }
    }

    #[test]
    fn parse_sheet_cell_ref() {
        let node = FormulaParser::parse("=Sheet1!B2").unwrap();
        match node {
            FormulaNode::CellRef(r) => {
                assert_eq!(r.sheet, Some("Sheet1".into()));
                assert_eq!(r.column, "B");
                assert_eq!(r.row, 2);
            }
            _ => panic!("expected CellRef"),
        }
    }

    #[test]
    fn parse_addition() {
        let node = FormulaParser::parse("=1+2").unwrap();
        match node {
            FormulaNode::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                assert_eq!(*left, FormulaNode::Literal(FormulaValue::Number(1.0)));
                assert_eq!(*right, FormulaNode::Literal(FormulaValue::Number(2.0)));
            }
            _ => panic!("expected BinaryOp Add"),
        }
    }

    #[test]
    fn parse_precedence_add_mul() {
        // 1+2*3 should be 1+(2*3)
        let node = FormulaParser::parse("=1+2*3").unwrap();
        match node {
            FormulaNode::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                assert_eq!(*left, FormulaNode::Literal(FormulaValue::Number(1.0)));
                match *right {
                    FormulaNode::BinaryOp {
                        op: BinaryOp::Mul,
                        left: ml,
                        right: mr,
                    } => {
                        assert_eq!(*ml, FormulaNode::Literal(FormulaValue::Number(2.0)));
                        assert_eq!(*mr, FormulaNode::Literal(FormulaValue::Number(3.0)));
                    }
                    _ => panic!("expected Mul on right"),
                }
            }
            _ => panic!("expected Add"),
        }
    }

    #[test]
    fn parse_parentheses_override_precedence() {
        // (1+2)*3 should be (1+2)*3
        let node = FormulaParser::parse("=(1+2)*3").unwrap();
        match node {
            FormulaNode::BinaryOp {
                op: BinaryOp::Mul,
                left,
                right,
            } => {
                match *left {
                    FormulaNode::Parenthesized(inner) => match *inner {
                        FormulaNode::BinaryOp {
                            op: BinaryOp::Add, ..
                        } => {}
                        _ => panic!("expected Add inside parens"),
                    },
                    _ => panic!("expected Parenthesized"),
                }
                assert_eq!(*right, FormulaNode::Literal(FormulaValue::Number(3.0)));
            }
            _ => panic!("expected Mul"),
        }
    }

    #[test]
    fn parse_exponentiation_right_associative() {
        // 2^3^4 should be 2^(3^4)
        let node = FormulaParser::parse("=2^3^4").unwrap();
        match node {
            FormulaNode::BinaryOp {
                op: BinaryOp::Pow,
                left,
                right,
            } => {
                assert_eq!(*left, FormulaNode::Literal(FormulaValue::Number(2.0)));
                match *right {
                    FormulaNode::BinaryOp {
                        op: BinaryOp::Pow, ..
                    } => {}
                    _ => panic!("expected Pow on right (right-associative)"),
                }
            }
            _ => panic!("expected Pow"),
        }
    }

    #[test]
    fn parse_unary_minus() {
        let node = FormulaParser::parse("=-5").unwrap();
        match node {
            FormulaNode::UnaryOp {
                op: UnaryOp::Neg,
                operand,
            } => {
                assert_eq!(*operand, FormulaNode::Literal(FormulaValue::Number(5.0)));
            }
            _ => panic!("expected UnaryOp Neg"),
        }
    }

    #[test]
    fn parse_postfix_percent() {
        let node = FormulaParser::parse("=50%").unwrap();
        match node {
            FormulaNode::UnaryOp {
                op: UnaryOp::Percent,
                operand,
            } => {
                assert_eq!(*operand, FormulaNode::Literal(FormulaValue::Number(50.0)));
            }
            _ => panic!("expected UnaryOp Percent"),
        }
    }

    #[test]
    fn parse_function_call() {
        let node = FormulaParser::parse("=SUM(1, 2, 3)").unwrap();
        match node {
            FormulaNode::FunctionCall { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected FunctionCall"),
        }
    }

    #[test]
    fn parse_nested_function() {
        let node = FormulaParser::parse("=SUM(1, MAX(2, 3))").unwrap();
        match node {
            FormulaNode::FunctionCall { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 2);
                match &args[1] {
                    FormulaNode::FunctionCall { name, args } => {
                        assert_eq!(name, "MAX");
                        assert_eq!(args.len(), 2);
                    }
                    _ => panic!("expected nested FunctionCall"),
                }
            }
            _ => panic!("expected FunctionCall"),
        }
    }

    #[test]
    fn parse_range() {
        let node = FormulaParser::parse("=A1:B5").unwrap();
        match node {
            FormulaNode::Range { start, end } => {
                assert_eq!(start.column, "A");
                assert_eq!(start.row, 1);
                assert_eq!(end.column, "B");
                assert_eq!(end.row, 5);
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn parse_function_with_range() {
        let node = FormulaParser::parse("=SUM(A1:A10)").unwrap();
        match node {
            FormulaNode::FunctionCall { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], FormulaNode::Range { .. }));
            }
            _ => panic!("expected FunctionCall"),
        }
    }

    #[test]
    fn parse_comparison() {
        let node = FormulaParser::parse("=A1>0").unwrap();
        assert!(matches!(
            node,
            FormulaNode::BinaryOp {
                op: BinaryOp::Gt,
                ..
            }
        ));
    }

    #[test]
    fn parse_concat() {
        let node = FormulaParser::parse(r#"="a"&"b""#).unwrap();
        assert!(matches!(
            node,
            FormulaNode::BinaryOp {
                op: BinaryOp::Concat,
                ..
            }
        ));
    }

    #[test]
    fn parse_complex_if() {
        let node = FormulaParser::parse("=IF(A1>0, SUM(B1:B10), 0)").unwrap();
        match node {
            FormulaNode::FunctionCall { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected IF function call"),
        }
    }

    #[test]
    fn parse_multiple_comparisons() {
        // A1>B1=TRUE should parse as (A1>B1)=TRUE (left-to-right)
        let node = FormulaParser::parse("=A1>B1=TRUE").unwrap();
        match node {
            FormulaNode::BinaryOp {
                op: BinaryOp::Eq, ..
            } => {}
            _ => panic!("expected Eq at top level"),
        }
    }

    #[test]
    fn parse_empty_function_args() {
        let node = FormulaParser::parse("=NOW()").unwrap();
        match node {
            FormulaNode::FunctionCall { name, args } => {
                assert_eq!(name, "NOW");
                assert!(args.is_empty());
            }
            _ => panic!("expected FunctionCall"),
        }
    }

    #[test]
    fn parse_cell_ref_function_validates() {
        let result = parse_cell_ref("A0");
        assert!(result.is_err()); // row 0 is invalid

        let result = parse_cell_ref("$A$1");
        assert!(result.is_ok());

        let result = parse_cell_ref("Sheet1!$B$2");
        let r = result.unwrap();
        assert_eq!(r.sheet, Some("Sheet1".into()));
        assert!(r.abs_column);
        assert!(r.abs_row);
    }

    #[test]
    fn parse_unary_plus() {
        let node = FormulaParser::parse("=+5").unwrap();
        assert_eq!(node, FormulaNode::Literal(FormulaValue::Number(5.0)));
    }
}
