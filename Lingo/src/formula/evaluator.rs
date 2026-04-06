use super::ast::{BinaryOp, FormulaCellRef, FormulaNode, UnaryOp};
use super::functions::FunctionRegistry;
use super::value::{FormulaErrorKind, FormulaValue};

/// Trait for resolving cell references during formula evaluation.
///
/// The spreadsheet layer implements this to provide cell values to the
/// formula engine without Lingo needing to know about Ideas or any
/// specific data model.
pub trait CellResolver: Send + Sync {
    /// Resolve a single cell reference to its current value.
    fn resolve(&self, cell_ref: &FormulaCellRef) -> FormulaValue;

    /// Resolve a rectangular range of cells to a flat list of values
    /// (row-major order).
    fn resolve_range(&self, start: &FormulaCellRef, end: &FormulaCellRef) -> Vec<FormulaValue>;
}

/// Evaluates a parsed formula AST against cell data.
pub struct FormulaEvaluator {
    functions: FunctionRegistry,
}

impl FormulaEvaluator {
    /// Create an evaluator with the default built-in functions.
    pub fn new() -> Self {
        Self {
            functions: FunctionRegistry::with_defaults(),
        }
    }

    /// Create an evaluator with a custom function registry.
    pub fn with_functions(registry: FunctionRegistry) -> Self {
        Self {
            functions: registry,
        }
    }

    /// Evaluate a formula AST node, resolving cell references via the resolver.
    pub fn evaluate(&self, node: &FormulaNode, resolver: &dyn CellResolver) -> FormulaValue {
        match node {
            FormulaNode::Literal(value) => value.clone(),

            FormulaNode::CellRef(cell_ref) => resolver.resolve(cell_ref),

            FormulaNode::Range { start, end } => {
                // A bare range outside a function call returns the first value,
                // matching spreadsheet convention.
                let values = resolver.resolve_range(start, end);
                values.into_iter().next().unwrap_or(FormulaValue::Empty)
            }

            FormulaNode::BinaryOp { op, left, right } => {
                let left_val = self.evaluate(left, resolver);
                let right_val = self.evaluate(right, resolver);
                self.apply_binary_op(op, &left_val, &right_val)
            }

            FormulaNode::UnaryOp { op, operand } => {
                let val = self.evaluate(operand, resolver);
                self.apply_unary_op(op, &val)
            }

            FormulaNode::FunctionCall { name, args } => {
                self.apply_function_call(name, args, resolver)
            }

            FormulaNode::Parenthesized(inner) => self.evaluate(inner, resolver),
        }
    }

    fn apply_binary_op(
        &self,
        op: &BinaryOp,
        left: &FormulaValue,
        right: &FormulaValue,
    ) -> FormulaValue {
        // Error propagation: if either operand is an error, propagate it.
        if let FormulaValue::Error(e) = left {
            return FormulaValue::Error(e.clone());
        }
        if let FormulaValue::Error(e) = right {
            return FormulaValue::Error(e.clone());
        }

        match op {
            BinaryOp::Add => match (left.as_number(), right.as_number()) {
                (Some(a), Some(b)) => FormulaValue::Number(a + b),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Sub => match (left.as_number(), right.as_number()) {
                (Some(a), Some(b)) => FormulaValue::Number(a - b),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Mul => match (left.as_number(), right.as_number()) {
                (Some(a), Some(b)) => FormulaValue::Number(a * b),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Div => match (left.as_number(), right.as_number()) {
                (Some(_), Some(0.0)) => FormulaValue::Error(FormulaErrorKind::Div0),
                (Some(a), Some(b)) => FormulaValue::Number(a / b),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Mod => match (left.as_number(), right.as_number()) {
                (Some(_), Some(0.0)) => FormulaValue::Error(FormulaErrorKind::Div0),
                (Some(a), Some(b)) => FormulaValue::Number(a % b),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Pow => match (left.as_number(), right.as_number()) {
                (Some(a), Some(b)) => FormulaValue::Number(a.powf(b)),
                _ => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Eq => FormulaValue::Bool(values_equal(left, right)),
            BinaryOp::Ne => FormulaValue::Bool(!values_equal(left, right)),
            BinaryOp::Lt => match compare_values(left, right) {
                Some(std::cmp::Ordering::Less) => FormulaValue::Bool(true),
                Some(_) => FormulaValue::Bool(false),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Le => match compare_values(left, right) {
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal) => {
                    FormulaValue::Bool(true)
                }
                Some(_) => FormulaValue::Bool(false),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Gt => match compare_values(left, right) {
                Some(std::cmp::Ordering::Greater) => FormulaValue::Bool(true),
                Some(_) => FormulaValue::Bool(false),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Ge => match compare_values(left, right) {
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal) => {
                    FormulaValue::Bool(true)
                }
                Some(_) => FormulaValue::Bool(false),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
            BinaryOp::Concat => {
                let l = value_to_text(left);
                let r = value_to_text(right);
                FormulaValue::Text(format!("{}{}", l, r))
            }
        }
    }

    fn apply_unary_op(&self, op: &UnaryOp, val: &FormulaValue) -> FormulaValue {
        if let FormulaValue::Error(e) = val {
            return FormulaValue::Error(e.clone());
        }

        match op {
            UnaryOp::Neg => match val.as_number() {
                Some(n) => FormulaValue::Number(-n),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
            UnaryOp::Percent => match val.as_number() {
                Some(n) => FormulaValue::Number(n / 100.0),
                None => FormulaValue::Error(FormulaErrorKind::Value),
            },
        }
    }

    fn apply_function_call(
        &self,
        name: &str,
        args: &[FormulaNode],
        resolver: &dyn CellResolver,
    ) -> FormulaValue {
        let (func, _expected_count) = match self.functions.get(name) {
            Some(entry) => *entry,
            None => return FormulaValue::Error(FormulaErrorKind::Name),
        };

        // Flatten range arguments into individual values for aggregate functions.
        let mut flat_args = Vec::new();
        for arg in args {
            match arg {
                FormulaNode::Range { start, end } => {
                    let range_vals = resolver.resolve_range(start, end);
                    flat_args.extend(range_vals);
                }
                _ => {
                    flat_args.push(self.evaluate(arg, resolver));
                }
            }
        }

        func(&flat_args)
    }
}

impl Default for FormulaEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Compare two FormulaValues for equality (used by = and <> operators).
fn values_equal(a: &FormulaValue, b: &FormulaValue) -> bool {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => (x - y).abs() < f64::EPSILON,
        (FormulaValue::Text(x), FormulaValue::Text(y)) => {
            x.to_lowercase() == y.to_lowercase()
        }
        (FormulaValue::Bool(x), FormulaValue::Bool(y)) => x == y,
        (FormulaValue::Empty, FormulaValue::Empty) => true,
        (FormulaValue::Empty, FormulaValue::Number(n))
        | (FormulaValue::Number(n), FormulaValue::Empty) => *n == 0.0,
        (FormulaValue::Empty, FormulaValue::Text(s))
        | (FormulaValue::Text(s), FormulaValue::Empty) => s.is_empty(),
        _ => false,
    }
}

/// Compare two FormulaValues for ordering.
fn compare_values(a: &FormulaValue, b: &FormulaValue) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (FormulaValue::Number(x), FormulaValue::Number(y)) => x.partial_cmp(y),
        (FormulaValue::Text(x), FormulaValue::Text(y)) => Some(x.cmp(y)),
        (FormulaValue::Bool(x), FormulaValue::Bool(y)) => Some(x.cmp(y)),
        (FormulaValue::Empty, FormulaValue::Number(n)) => 0.0_f64.partial_cmp(n),
        (FormulaValue::Number(n), FormulaValue::Empty) => n.partial_cmp(&0.0),
        _ => None,
    }
}

/// Convert a FormulaValue to its text representation for concatenation.
fn value_to_text(v: &FormulaValue) -> String {
    match v {
        FormulaValue::Text(s) => s.clone(),
        FormulaValue::Number(n) => {
            if n.fract() == 0.0 && n.is_finite() {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        FormulaValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        FormulaValue::Empty => String::new(),
        FormulaValue::Error(e) => e.to_string(),
        FormulaValue::Date(dt) => dt.format("%Y-%m-%d").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parser::FormulaParser;
    use std::collections::HashMap;

    /// A mock CellResolver backed by a HashMap.
    struct MockResolver {
        cells: HashMap<String, FormulaValue>,
    }

    impl MockResolver {
        fn new() -> Self {
            Self {
                cells: HashMap::new(),
            }
        }

        fn set(&mut self, cell: &str, value: FormulaValue) {
            self.cells.insert(cell.to_uppercase(), value);
        }
    }

    impl CellResolver for MockResolver {
        fn resolve(&self, cell_ref: &FormulaCellRef) -> FormulaValue {
            let key = format!("{}{}", cell_ref.column, cell_ref.row);
            self.cells.get(&key).cloned().unwrap_or(FormulaValue::Empty)
        }

        fn resolve_range(
            &self,
            start: &FormulaCellRef,
            end: &FormulaCellRef,
        ) -> Vec<FormulaValue> {
            let start_col = start.column_index();
            let end_col = end.column_index();
            let start_row = start.row;
            let end_row = end.row;

            let mut values = Vec::new();
            for row in start_row..=end_row {
                for col in start_col..=end_col {
                    let cell = FormulaCellRef::from_indices(col, row);
                    values.push(self.resolve(&cell));
                }
            }
            values
        }
    }

    fn run_formula(formula: &str, resolver: &dyn CellResolver) -> FormulaValue {
        let ast = FormulaParser::parse(formula).unwrap();
        let evaluator = FormulaEvaluator::new();
        evaluator.evaluate(&ast, resolver)
    }

    #[test]
    fn evaluate_literal_number() {
        let resolver = MockResolver::new();
        assert_eq!(run_formula("=42", &resolver), FormulaValue::Number(42.0));
    }

    #[test]
    fn evaluate_literal_string() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula(r#"="hello""#, &resolver),
            FormulaValue::Text("hello".into())
        );
    }

    #[test]
    fn evaluate_arithmetic() {
        let resolver = MockResolver::new();
        assert_eq!(run_formula("=1+2*3", &resolver), FormulaValue::Number(7.0));
        assert_eq!(run_formula("=(1+2)*3", &resolver), FormulaValue::Number(9.0));
        assert_eq!(run_formula("=10/4", &resolver), FormulaValue::Number(2.5));
        assert_eq!(run_formula("=2^10", &resolver), FormulaValue::Number(1024.0));
    }

    #[test]
    fn evaluate_division_by_zero() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula("=1/0", &resolver),
            FormulaValue::Error(FormulaErrorKind::Div0)
        );
    }

    #[test]
    fn evaluate_unary_minus() {
        let resolver = MockResolver::new();
        assert_eq!(run_formula("=-5", &resolver), FormulaValue::Number(-5.0));
        assert_eq!(run_formula("=--5", &resolver), FormulaValue::Number(5.0));
    }

    #[test]
    fn evaluate_percent() {
        let resolver = MockResolver::new();
        assert_eq!(run_formula("=50%", &resolver), FormulaValue::Number(0.5));
    }

    #[test]
    fn evaluate_comparison() {
        let resolver = MockResolver::new();
        assert_eq!(run_formula("=1=1", &resolver), FormulaValue::Bool(true));
        assert_eq!(run_formula("=1=2", &resolver), FormulaValue::Bool(false));
        assert_eq!(run_formula("=1<2", &resolver), FormulaValue::Bool(true));
        assert_eq!(run_formula("=2<=2", &resolver), FormulaValue::Bool(true));
        assert_eq!(run_formula("=3>2", &resolver), FormulaValue::Bool(true));
        assert_eq!(run_formula("=2>=3", &resolver), FormulaValue::Bool(false));
        assert_eq!(run_formula("=1<>2", &resolver), FormulaValue::Bool(true));
    }

    #[test]
    fn evaluate_string_concat() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula(r#"="hello"&" "&"world""#, &resolver),
            FormulaValue::Text("hello world".into())
        );
    }

    #[test]
    fn evaluate_cell_ref() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(10.0));
        resolver.set("B1", FormulaValue::Number(20.0));
        assert_eq!(run_formula("=A1+B1", &resolver), FormulaValue::Number(30.0));
    }

    #[test]
    fn evaluate_sum_range() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(1.0));
        resolver.set("A2", FormulaValue::Number(2.0));
        resolver.set("A3", FormulaValue::Number(3.0));
        assert_eq!(run_formula("=SUM(A1:A3)", &resolver), FormulaValue::Number(6.0));
    }

    #[test]
    fn evaluate_average_range() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(2.0));
        resolver.set("A2", FormulaValue::Number(4.0));
        resolver.set("A3", FormulaValue::Number(6.0));
        assert_eq!(
            run_formula("=AVERAGE(A1:A3)", &resolver),
            FormulaValue::Number(4.0)
        );
    }

    #[test]
    fn evaluate_if_function() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(5.0));
        assert_eq!(
            run_formula(r#"=IF(A1>0, "positive", "negative")"#, &resolver),
            FormulaValue::Text("positive".into())
        );

        resolver.set("A1", FormulaValue::Number(-1.0));
        assert_eq!(
            run_formula(r#"=IF(A1>0, "positive", "negative")"#, &resolver),
            FormulaValue::Text("negative".into())
        );
    }

    #[test]
    fn evaluate_nested_functions() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(1.0));
        resolver.set("A2", FormulaValue::Number(2.0));
        resolver.set("B1", FormulaValue::Number(10.0));
        resolver.set("B2", FormulaValue::Number(20.0));

        // SUM(A1:A2) + MAX(B1:B2) = 3 + 20 = 23
        assert_eq!(
            run_formula("=SUM(A1:A2)+MAX(B1:B2)", &resolver),
            FormulaValue::Number(23.0)
        );
    }

    #[test]
    fn evaluate_error_propagation() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Error(FormulaErrorKind::Ref));
        assert_eq!(
            run_formula("=A1+1", &resolver),
            FormulaValue::Error(FormulaErrorKind::Ref)
        );
    }

    #[test]
    fn evaluate_unknown_function() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula("=VLOOKUP(1)", &resolver),
            FormulaValue::Error(FormulaErrorKind::Name)
        );
    }

    #[test]
    fn evaluate_concat_with_numbers() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula(r#"="count: "&42"#, &resolver),
            FormulaValue::Text("count: 42".into())
        );
    }

    #[test]
    fn evaluate_empty_cell() {
        let resolver = MockResolver::new();
        // A1 not set -> Empty, Empty.as_number() is None -> #VALUE!
        let result = run_formula("=A1+5", &resolver);
        assert_eq!(result, FormulaValue::Error(FormulaErrorKind::Value));
    }

    #[test]
    fn evaluate_min_max_range() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(5.0));
        resolver.set("A2", FormulaValue::Number(2.0));
        resolver.set("A3", FormulaValue::Number(8.0));

        assert_eq!(run_formula("=MIN(A1:A3)", &resolver), FormulaValue::Number(2.0));
        assert_eq!(run_formula("=MAX(A1:A3)", &resolver), FormulaValue::Number(8.0));
    }

    #[test]
    fn evaluate_count_range() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(1.0));
        resolver.set("A2", FormulaValue::Text("hi".into()));
        resolver.set("A3", FormulaValue::Number(3.0));

        assert_eq!(
            run_formula("=COUNT(A1:A3)", &resolver),
            FormulaValue::Number(2.0)
        );
    }

    #[test]
    fn evaluate_text_functions() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula(r#"=LEN("hello")"#, &resolver),
            FormulaValue::Number(5.0)
        );
        assert_eq!(
            run_formula(r#"=UPPER("hello")"#, &resolver),
            FormulaValue::Text("HELLO".into())
        );
        assert_eq!(
            run_formula(r#"=LEFT("hello", 3)"#, &resolver),
            FormulaValue::Text("hel".into())
        );
    }

    #[test]
    fn evaluate_and_or() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula("=AND(TRUE, TRUE)", &resolver),
            FormulaValue::Bool(true)
        );
        assert_eq!(
            run_formula("=AND(TRUE, FALSE)", &resolver),
            FormulaValue::Bool(false)
        );
        assert_eq!(
            run_formula("=OR(FALSE, TRUE)", &resolver),
            FormulaValue::Bool(true)
        );
        assert_eq!(
            run_formula("=OR(FALSE, FALSE)", &resolver),
            FormulaValue::Bool(false)
        );
    }

    #[test]
    fn evaluate_complex_formula() {
        let mut resolver = MockResolver::new();
        resolver.set("A1", FormulaValue::Number(100.0));
        resolver.set("A2", FormulaValue::Number(200.0));
        resolver.set("A3", FormulaValue::Number(300.0));

        // IF(SUM(A1:A3) > 500, "big", "small")
        assert_eq!(
            run_formula(r#"=IF(SUM(A1:A3)>500, "big", "small")"#, &resolver),
            FormulaValue::Text("big".into())
        );
    }

    #[test]
    fn evaluator_default() {
        let evaluator = FormulaEvaluator::default();
        let resolver = MockResolver::new();
        let ast = FormulaParser::parse("=1+2").unwrap();
        assert_eq!(
            evaluator.evaluate(&ast, &resolver),
            FormulaValue::Number(3.0)
        );
    }

    #[test]
    fn evaluate_string_equality_case_insensitive() {
        let resolver = MockResolver::new();
        assert_eq!(
            run_formula(r#"="Hello"="hello""#, &resolver),
            FormulaValue::Bool(true)
        );
    }
}
