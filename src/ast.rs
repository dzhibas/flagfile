use core::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    String(String),
    Number(i64),
    Float(f64),
    Boolean(bool),
    Variable(String),
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::String(string) => write!(f, "{string}"),
            Atom::Number(number) => write!(f, "{number}"), 
            Atom::Float(float) => write!(f, "{float}"),
            Atom::Boolean(bool) => write!(f, "{bool}"),
            Atom::Variable(var) => write!(f, "{var}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ComparisonOp {
    Eq,
    More,
    Less,
    MoreEq,
    LessEq,
    NotEq,
}

impl ComparisonOp {
    fn from_str(expr: &str) -> Self {
        match expr {
            "==" | "=" => ComparisonOp::Eq,
            ">" => ComparisonOp::More,
            ">=" => ComparisonOp::MoreEq,
            "<" => ComparisonOp::Less,
            "<=" => ComparisonOp::LessEq,
            "!=" | "<>" => ComparisonOp::NotEq,
            _ => unreachable!(),
        }
    }
}
impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self) {
            ComparisonOp::Eq => write!(f, "=="),
            ComparisonOp::More => write!(f, ">"),
            ComparisonOp::Less => write!(f, "<"), 
            ComparisonOp::MoreEq => write!(f, ">="),
            ComparisonOp::LessEq => write!(f, "<="),
            ComparisonOp::NotEq => write!(f, "<>"),
        }
    }
}

pub enum AstNode {
}
