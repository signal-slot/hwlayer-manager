//! Structured one-line records: `kind key=value key=value ...`.
//!
//! Values that are empty or contain white space, `"`, `\` or `=` are written
//! in double quotes with `"` and `\` escaped, so that a line splits into
//! fields with a simple shell-like tokenizer.

use std::borrow::Cow;
use std::fmt::{self, Display};

/// One output line.
#[derive(Clone, Debug)]
pub struct Record {
    line: String,
}

impl Record {
    /// Starts a record of the given kind (the first token of the line).
    pub fn new(kind: &str) -> Self {
        Self {
            line: kind.to_string(),
        }
    }

    /// Appends `key=value`.
    pub fn field(mut self, key: &str, value: impl Display) -> Self {
        self.line.push(' ');
        self.line.push_str(key);
        self.line.push('=');
        self.line.push_str(&quote(&value.to_string()));
        self
    }

    /// Appends `key=value` if there is a value.
    pub fn field_opt(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(value) => self.field(key, value),
            None => self,
        }
    }

    /// Prints the record on standard output.
    pub fn emit(self) {
        println!("{}", self.line);
    }
}

impl Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line)
    }
}

/// Quotes a value if it would not survive splitting at white space and `=`.
pub fn quote(value: &str) -> Cow<'_, str> {
    let plain = !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\\' || c == '=');
    if plain {
        return Cow::Borrowed(value);
    }
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        match c {
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    Cow::Owned(quoted)
}

/// The final line of a scenario: `VERDICT <scenario> PASS|FAIL <reason>`.
///
/// The reason is free text to the end of the line, on one line.
pub fn verdict_line(scenario: &str, pass: bool, reason: &str) -> String {
    let reason: String = reason
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!(
        "VERDICT {scenario} {} {}",
        if pass { "PASS" } else { "FAIL" },
        reason.trim()
    )
}

/// Joins values with commas, for list-valued fields such as plane ids.
pub fn list<T: Display>(items: impl IntoIterator<Item = T>) -> String {
    let mut out = String::new();
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&item.to_string());
    }
    if out.is_empty() {
        out.push('-');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_fields() {
        let r = Record::new("frame").field("n", 3).field("plane", 31);
        assert_eq!(r.to_string(), "frame n=3 plane=31");
    }

    #[test]
    fn quoted_fields() {
        assert_eq!(quote("LVDS-1"), "LVDS-1");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("x=y"), "\"x=y\"");
        assert_eq!(quote("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quote("a\\b"), "\"a\\\\b\"");
        assert_eq!(quote("two\nlines"), "\"two\\nlines\"");
        let r = Record::new("error").field("message", "Permission denied (os error 13)");
        assert_eq!(
            r.to_string(),
            "error message=\"Permission denied (os error 13)\""
        );
    }

    #[test]
    fn optional_fields() {
        let r = Record::new("x")
            .field_opt("a", Some(1))
            .field_opt("b", None::<u32>);
        assert_eq!(r.to_string(), "x a=1");
    }

    #[test]
    fn verdicts() {
        assert_eq!(
            verdict_line("two-planes", true, "2 planes: 31,32"),
            "VERDICT two-planes PASS 2 planes: 31,32"
        );
        assert_eq!(
            verdict_line("rotate", false, "line one\nline two "),
            "VERDICT rotate FAIL line one line two"
        );
    }

    #[test]
    fn lists() {
        assert_eq!(list([31, 32]), "31,32");
        assert_eq!(list(Vec::<u32>::new()), "-");
    }
}
