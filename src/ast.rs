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

pub enum AstNode {
}
