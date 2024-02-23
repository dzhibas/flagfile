use core::fmt;

use chrono::NaiveDate;

/// TODO: add date and datetime as its common
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    String(String),
    Number(i32),
    Float(f64),
    Boolean(bool),
    Variable(String),
    Date(NaiveDate),
    DateTime(String),
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self {
            Atom::Number(v) => match other {
                Atom::Number(v2) => Some(v.cmp(v2)),
                Atom::Float(v2) => f64::from(*v).partial_cmp(v2),
                _ => None,
            },
            Atom::Float(v) => match other {
                Atom::Float(v2) => v.partial_cmp(v2),
                Atom::Number(v2) => v.partial_cmp(&f64::from(*v2)),
                _ => None,
            },
            _ => None,
        }
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::String(string) => write!(f, "{string}"),
            Atom::Number(number) => write!(f, "{number}"),
            Atom::Float(float) => write!(f, "{float}"),
            Atom::Boolean(bool) => write!(f, "{bool}"),
            Atom::Variable(var) => write!(f, "{var}"),
            Atom::Date(var) => write!(f, "{var}"),
            Atom::DateTime(var) => write!(f, "{var}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Eq,
    More,
    Less,
    MoreEq,
    LessEq,
    NotEq,
}

impl ComparisonOp {
    pub fn from_str(expr: &str) -> Self {
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

#[derive(Debug, Clone, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

impl LogicOp {
    pub fn from_str(i: &str) -> Self {
        match i.to_lowercase().as_str() {
            "and" | "&&" => LogicOp::And,
            "or" | "||" => LogicOp::Or,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayOp {
    In,
    NotIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnCall {
    Upper,
    Lower,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Void,
    Variable(Atom),
    Function(FnCall, Box<AstNode>),
    Constant(Atom),
    List(Vec<Atom>),
    Compare(Box<AstNode>, ComparisonOp, Box<AstNode>),
    Array(Box<AstNode>, ArrayOp, Box<AstNode>),
    Logic(Box<AstNode>, LogicOp, Box<AstNode>),
    Scope { expr: Box<AstNode>, negate: bool },
}

impl AstNode {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AstNode::Variable(Atom::Variable(s)) => Some(s.as_str()),
            AstNode::Constant(Atom::String(s)) => Some(s.as_str()),
            AstNode::Constant(Atom::Variable(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}
