use core::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Atom(String),
    I64(i64),
    String(String),
    List(Vec<Value>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Atom(s) => write!(f, "{s}"),
            Value::I64(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "\"{s}\""),
            Value::List(list) => {
                write!(f, "(")?;
                for (i, val) in list.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("unmatched closing parenthesis")]
    UnmatchedCloseParen,
    #[error("unterminated string literal")]
    UnterminatedString,
    #[error("unclosed parentheses: {0} unclosed")]
    UnclosedParens(usize),
    #[error("multiple top-level forms not allowed")]
    MultipleTopLevelForms,
    #[error(transparent)]
    InvalidInteger(std::num::ParseIntError),
}

#[derive(Debug)]
enum State {
    Normal,
    InString { buf: String, escaped: bool },
}

#[derive(Debug)]
pub struct Parser {
    state: State,
    current_atom: String,
    stack: Vec<Vec<Value>>,
    output: Vec<Value>,
    /// Track if we're inside the outer list
    /// True means we've opened the outer `(` and are streaming its children
    in_outer_list: bool,
}

/// Convenience function to parse a complete s-expression from a &str
pub fn parse_sexpr(input: &str) -> Result<Vec<Value>, Error> {
    let mut parser = Parser::new();
    parser.take(input)?;
    parser.finish()
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            current_atom: String::new(),
            stack: Vec::new(),
            output: Vec::new(),
            in_outer_list: false,
        }
    }

    pub fn take(&mut self, chunk: &str) -> Result<(), Error> {
        for ch in chunk.chars() {
            match &mut self.state {
                State::InString { buf, escaped } => {
                    if *escaped {
                        buf.push(match ch {
                            'n' => '\n',
                            't' => '\t',
                            other => other, // \", \\, or passthrough
                        });
                        *escaped = false;
                    } else {
                        match ch {
                            '\\' => *escaped = true,
                            '"' => {
                                let s = std::mem::take(buf);
                                self.push_value(Value::String(s));
                                self.state = State::Normal;
                            }
                            c => buf.push(c),
                        }
                    }
                }
                State::Normal => match ch {
                    '"' => {
                        self.flush_atom()?;
                        self.state = State::InString {
                            buf: String::new(),
                            escaped: false,
                        };
                    }
                    '(' => {
                        self.flush_atom()?;

                        // Reject multiple top-level forms
                        if self.stack.is_empty() && self.in_outer_list {
                            return Err(Error::MultipleTopLevelForms);
                        }

                        // Track if this is the outer list
                        if self.stack.is_empty() {
                            self.in_outer_list = true;
                        }

                        self.stack.push(Vec::new());
                    }
                    ')' => {
                        self.flush_atom()?;
                        let list = self.stack.pop().ok_or(Error::UnmatchedCloseParen)?;

                        // If we just closed the outer list, don't push it (we've streamed its children)
                        if self.stack.is_empty() && self.in_outer_list {
                            // Outer list closed - we're done
                        } else {
                            self.push_value(Value::List(list));
                        }
                    }
                    c if c.is_whitespace() => {
                        self.flush_atom()?;
                    }
                    c => self.current_atom.push(c),
                },
            }
        }
        Ok(())
    }

    fn flush_atom(&mut self) -> Result<(), Error> {
        if self.current_atom.is_empty() {
            Ok(())
        } else if matches!(self.current_atom.chars().next(), Some('-' | '0'..='9')) {
            let num = self
                .current_atom
                .parse::<i64>()
                .map_err(Error::InvalidInteger)?;
            self.push_value(Value::I64(num));
            self.current_atom.clear();
            Ok(())
        } else {
            let atom = std::mem::take(&mut self.current_atom);
            self.push_value(Value::Atom(atom));
            Ok(())
        }
    }

    fn push_value(&mut self, value: Value) {
        // If we're at depth 1 (inside the outer list), yield immediately
        if self.stack.len() == 1 && self.in_outer_list {
            self.output.push(value);
        } else {
            match self.stack.last_mut() {
                Some(parent) => parent.push(value),
                None => self.output.push(value),
            }
        }
    }

    /// Drain any completed s-expressions from the output
    /// Returns them and clears the output buffer
    /// This is useful for streaming parsing
    pub fn drain_output(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.output)
    }

    /// Call when input is done to check for errors
    pub fn finish(mut self) -> Result<Vec<Value>, Error> {
        self.flush_atom()?;
        if matches!(self.state, State::InString { .. }) {
            return Err(Error::UnterminatedString);
        }
        if !self.stack.is_empty() {
            return Err(Error::UnclosedParens(self.stack.len()));
        }
        Ok(self.output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_atom() {
        let mut parser = Parser::new();
        parser.take("(foo)").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Ok(vec![Value::Atom("foo".into())]));
    }

    #[test]
    fn test_parser_string() {
        let mut parser = Parser::new();
        parser.take("(\"bar baz\")").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Ok(vec![Value::String("bar baz".into())]));
    }

    #[test]
    fn test_parser_escaped_string() {
        let mut parser = Parser::new();
        parser.take("(\"escaped \\\"\")").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Ok(vec![Value::String("escaped \"".into())]));
    }

    #[test]
    fn test_parser_i64() {
        let mut parser = Parser::new();
        parser.take("(123 )").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Ok(vec![Value::I64(123)]));
    }

    #[test]
    fn test_parser_negative_i64() {
        let mut parser = Parser::new();
        parser.take("(-123 )").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Ok(vec![Value::I64(-123)]));
    }

    #[test]
    fn test_parser_nested() {
        let mut parser = Parser::new();
        parser
            .take("(foo (bar 42) \"baz\")")
            .expect("should succeed");
        let output = parser.finish();
        assert_eq!(
            output,
            Ok(vec![
                Value::Atom("foo".into()),
                Value::List(vec![Value::Atom("bar".into()), Value::I64(42)]),
                Value::String("baz".into()),
            ])
        );
    }

    #[test]
    fn test_parser_list() {
        let mut parser = Parser::new();
        let result = parser.take("(a)(1)");
        assert_eq!(result, Err(Error::MultipleTopLevelForms));
    }

    #[test]
    fn test_parser_invalid_integer() {
        let mut parser = Parser::new();
        let result = parser.take("(123abc)");
        assert!(matches!(result, Err(Error::InvalidInteger(_))));
    }

    #[test]
    fn test_parser_unmatched_close_paren() {
        let mut parser = Parser::new();
        let result = parser.take(")");
        assert_eq!(result, Err(Error::UnmatchedCloseParen));
    }

    #[test]
    fn test_parser_unterminated_string() {
        let mut parser = Parser::new();
        parser.take("(\"unterminated)").expect("should succeed");
        let output = parser.finish();
        assert_eq!(output, Err(Error::UnterminatedString));
    }
}
