use partiql_value::Value;

use std::fmt::Result;
use std::fmt::Write;

// TODO: can move this to `partiql-value`?
pub trait PrettyPrint {
    fn pretty(&self, f: &mut String) -> Result;
}

impl PrettyPrint for Value {
    fn pretty(&self, s: &mut String) -> Result {
        pretty(self, s, 0)
    }
}

fn pretty(value: &Value, s: &mut String, cur_indent_size: usize) -> Result {
    // For now, defining an indent to be two spaces. We could allow users to pass in an indent string
    // for more control.
    let indent = "  ".repeat(cur_indent_size);
    match value {
        Value::List(l) => {
            writeln!(s, "[")?;
            let mut iter = l.iter().peekable();
            while let Some(v) = iter.next() {
                if iter.peek().is_some() {
                    write!(s, "{indent}  ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                    writeln!(s, ",")?;
                } else {
                    write!(s, "{indent}  ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                }
            }
            write!(s, "\n{indent}]")
        }
        Value::Bag(b) => {
            writeln!(s, "<<")?;
            let mut iter = b.iter().peekable();
            while let Some(v) = iter.next() {
                if iter.peek().is_some() {
                    write!(s, "{indent}  ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                    writeln!(s, ",")?;
                } else {
                    write!(s, "{indent}  ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                }
            }
            write!(s, "\n{indent}>>")
        }
        Value::Tuple(t) => {
            writeln!(s, "{{")?;
            let mut iter = t.pairs().peekable();
            while let Some((k, v)) = iter.next() {
                if iter.peek().is_some() {
                    write!(s, "{indent}  {k}: ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                    writeln!(s, ",")?;
                } else {
                    write!(s, "{indent}  {k}: ")?;
                    pretty(v, s, cur_indent_size + 1)?;
                }
            }
            write!(s, "\n{indent}}}")
        }
        _ => write!(s, "{value:?}"),
    }
}
