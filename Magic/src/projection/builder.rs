/// Language-agnostic, indent-aware source code builder.
///
/// Ported from Swiftlight's CodeBuilder. All code generators use this
/// to produce clean, properly formatted output.
#[derive(Debug, Clone)]
pub struct CodeBuilder {
    lines: Vec<String>,
    indent_level: usize,
    indent_string: String,
}

impl CodeBuilder {
    /// Create a builder with 4-space indentation.
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            indent_level: 0,
            indent_string: "    ".into(), // 4 spaces
        }
    }

    /// Create a builder with a custom indentation string (e.g., a tab).
    pub fn with_indent(indent_string: impl Into<String>) -> Self {
        Self {
            lines: Vec::new(),
            indent_level: 0,
            indent_string: indent_string.into(),
        }
    }

    /// Emit an indented line.
    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        let prefix = self.indent_string.repeat(self.indent_level);
        self.lines.push(format!("{}{}", prefix, text.as_ref()));
        self
    }

    /// Emit a blank line.
    pub fn blank(&mut self) -> &mut Self {
        self.lines.push(String::new());
        self
    }

    /// Emit a comment line (// prefix).
    pub fn comment(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.line(format!("// {}", text.as_ref()))
    }

    /// Emit a doc comment line (/// prefix).
    pub fn doc_comment(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.line(format!("/// {}", text.as_ref()))
    }

    /// Run a block at increased indent level.
    pub fn indent(&mut self, body: impl FnOnce(&mut Self)) -> &mut Self {
        self.indent_level += 1;
        body(self);
        self.indent_level -= 1;
        self
    }

    /// Emit `header {`, run body indented, emit `}`.
    pub fn braced(&mut self, header: impl AsRef<str>, body: impl FnOnce(&mut Self)) -> &mut Self {
        self.line(format!("{} {{", header.as_ref()));
        self.indent(body);
        self.line("}");
        self
    }

    /// Emit `{`, run body indented, emit `} trailing`.
    pub fn braced_trailing(
        &mut self,
        trailing: impl AsRef<str>,
        body: impl FnOnce(&mut Self),
    ) -> &mut Self {
        self.line("{");
        self.indent(body);
        self.line(format!("}} {}", trailing.as_ref()));
        self
    }

    /// Emit a line only if condition is true.
    pub fn line_if(&mut self, condition: bool, text: impl AsRef<str>) -> &mut Self {
        if condition {
            self.line(text);
        }
        self
    }

    /// Get the final assembled output.
    pub fn output(&self) -> String {
        self.lines.join("\n")
    }

    /// Number of lines emitted so far.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Whether no lines have been emitted yet.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl Default for CodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder() {
        let b = CodeBuilder::new();
        assert!(b.is_empty());
        assert_eq!(b.output(), "");
    }

    #[test]
    fn single_line() {
        let mut b = CodeBuilder::new();
        b.line("let x = 5;");
        assert_eq!(b.output(), "let x = 5;");
    }

    #[test]
    fn blank_line() {
        let mut b = CodeBuilder::new();
        b.line("a");
        b.blank();
        b.line("b");
        assert_eq!(b.output(), "a\n\nb");
    }

    #[test]
    fn comment_prefix() {
        let mut b = CodeBuilder::new();
        b.comment("TODO: fix this");
        assert_eq!(b.output(), "// TODO: fix this");
    }

    #[test]
    fn doc_comment_prefix() {
        let mut b = CodeBuilder::new();
        b.doc_comment("A type.");
        assert_eq!(b.output(), "/// A type.");
    }

    #[test]
    fn indent_increases_level() {
        let mut b = CodeBuilder::new();
        b.line("outer");
        b.indent(|b| {
            b.line("inner");
        });
        b.line("outer again");
        assert_eq!(b.output(), "outer\n    inner\nouter again");
    }

    #[test]
    fn braced_block() {
        let mut b = CodeBuilder::new();
        b.braced("struct Foo", |b| {
            b.line("x: i32,");
        });
        assert_eq!(b.output(), "struct Foo {\n    x: i32,\n}");
    }

    #[test]
    fn braced_trailing() {
        let mut b = CodeBuilder::new();
        b.braced_trailing(".padding()", |b| {
            b.line("content");
        });
        assert_eq!(b.output(), "{\n    content\n} .padding()");
    }

    #[test]
    fn line_if_true_adds() {
        let mut b = CodeBuilder::new();
        b.line_if(true, "visible");
        b.line_if(false, "hidden");
        assert_eq!(b.output(), "visible");
        assert_eq!(b.line_count(), 1);
    }

    #[test]
    fn nested_indent() {
        let mut b = CodeBuilder::new();
        b.braced("fn main()", |b| {
            b.braced("if true", |b| {
                b.line("println!(\"deep\");");
            });
        });
        assert_eq!(
            b.output(),
            "fn main() {\n    if true {\n        println!(\"deep\");\n    }\n}"
        );
    }

    #[test]
    fn custom_indent_string() {
        let mut b = CodeBuilder::with_indent("\t");
        b.braced("fn f()", |b| {
            b.line("x");
        });
        assert_eq!(b.output(), "fn f() {\n\tx\n}");
    }

    #[test]
    fn line_count_tracking() {
        let mut b = CodeBuilder::new();
        b.line("a");
        b.blank();
        b.line("b");
        assert_eq!(b.line_count(), 3);
    }
}
