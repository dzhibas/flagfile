use chrono::NaiveDate;
use core::fmt;

use crate::parse::parse_atom;

#[derive(Debug, Clone)]
pub enum Atom {
    String(String),
    Number(i32),
    Float(f64),
    Boolean(bool),
    Variable(String),
    Date(NaiveDate),
    DateTime(String),
    Semver(u32, u32, u32),
    // Timestamp(i64)
}

/// Try to interpret a float as semver components (e.g. 5.4 → (5, 4, 0)).
fn float_to_semver(f: f64) -> Option<(u32, u32, u32)> {
    let s = format!("{}", f);
    if let Some((maj_s, min_s)) = s.split_once('.') {
        let maj = maj_s.parse::<u32>().ok()?;
        let min = min_s.parse::<u32>().ok()?;
        Some((maj, min, 0))
    } else {
        let maj = s.parse::<u32>().ok()?;
        Some((maj, 0, 0))
    }
}

impl PartialEq<Atom> for Atom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Atom::String(s1), Atom::String(s2)) => s1 == s2,
            (Atom::Variable(v1), Atom::Variable(v2)) => v1 == v2,
            (Atom::String(v1), Atom::Variable(v2)) => v1 == v2,
            (Atom::Variable(v1), Atom::String(v2)) => v1 == v2,
            (Atom::Number(n1), Atom::Number(n2)) => n1 == n2,
            (Atom::Float(f1), Atom::Float(f2)) => f1 == f2,
            (Atom::Boolean(b1), Atom::Boolean(b2)) => b1 == b2,
            (Atom::Date(d1), Atom::Date(d2)) => d1 == d2,
            (Atom::DateTime(t1), Atom::DateTime(t2)) => t1 == t2,
            (Atom::Semver(a1, b1, c1), Atom::Semver(a2, b2, c2)) => {
                a1 == a2 && b1 == b2 && c1 == c2
            }
            (Atom::Semver(a, b, c), Atom::Float(f))
            | (Atom::Float(f), Atom::Semver(a, b, c)) => {
                if let Some((maj, min, patch)) = float_to_semver(*f) {
                    *a == maj && *b == min && *c == patch
                } else {
                    false
                }
            }
            (Atom::Semver(a, b, c), Atom::Number(n))
            | (Atom::Number(n), Atom::Semver(a, b, c)) => {
                if *n >= 0 {
                    *a == *n as u32 && *b == 0 && *c == 0
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self {
            Atom::Number(v) => match other {
                Atom::Number(v2) => Some(v.cmp(v2)),
                Atom::Float(v2) => f64::from(*v).partial_cmp(v2),
                Atom::Semver(a2, b2, c2) => {
                    if *v < 0 { return None; }
                    let maj = *v as u32;
                    Some(maj.cmp(a2).then(0u32.cmp(b2)).then(0u32.cmp(c2)))
                }
                _ => None,
            },
            Atom::Float(v) => match other {
                Atom::Float(v2) => v.partial_cmp(v2),
                Atom::Number(v2) => v.partial_cmp(&f64::from(*v2)),
                Atom::Semver(a2, b2, c2) => {
                    let (maj, min, patch) = float_to_semver(*v)?;
                    Some(maj.cmp(a2).then(min.cmp(b2)).then(patch.cmp(c2)))
                }
                _ => None,
            },
            Atom::Date(v) => match other {
                Atom::Date(v2) => v.partial_cmp(v2),
                // TODO: if compare to number it might be unix-timestamp
                _ => None,
            },
            Atom::Semver(a1, b1, c1) => match other {
                Atom::Semver(a2, b2, c2) => {
                    Some(a1.cmp(a2).then(b1.cmp(b2)).then(c1.cmp(c2)))
                }
                Atom::Float(f) => {
                    let (maj, min, patch) = float_to_semver(*f)?;
                    Some(a1.cmp(&maj).then(b1.cmp(&min)).then(c1.cmp(&patch)))
                }
                Atom::Number(n) => {
                    if *n < 0 { return None; }
                    let maj = *n as u32;
                    Some(a1.cmp(&maj).then(b1.cmp(&0).then(c1.cmp(&0))))
                }
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
            Atom::Semver(major, minor, patch) => write!(f, "{major}.{minor}.{patch}"),
        }
    }
}

impl<'a> From<&'a str> for Atom {
    fn from(val: &'a str) -> Self {
        let res = parse_atom(val);
        if let Ok((_i, out)) = res {
            return out;
        }
        Atom::String(val.into())
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
    // RegexMatch =~
    // NotRegexMatch !=~
    // Contains =*
    // DoesNotContain !=*
}

impl ComparisonOp {
    pub fn build_from_str(expr: &str) -> Self {
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
        match self {
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
    pub fn build_from_str(i: &str) -> Self {
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
    Now,
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
