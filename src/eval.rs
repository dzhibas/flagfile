use std::collections::HashMap;

use crate::ast::{AstNode, Atom, ComparisonOp};

pub fn eval<'a>(expr: &AstNode, context: &HashMap<&str, Atom>) -> Result<bool, &'a str> {
    let mut result = false;
    result = match expr {
        AstNode::Compare(var, op, val) => {
            // check var in context
            // compare with val
            let context_val = context.get(var.as_str().unwrap());
            let val_content = match val.as_ref() {
                AstNode::Constant(a) => Some(a),
                _ => None,
            }
            .unwrap();

            if let Some(c_val) = context_val {
                match op {
                    ComparisonOp::More => c_val > val_content,
                    ComparisonOp::MoreEq => c_val >= val_content,
                    ComparisonOp::Less => c_val < val_content,
                    ComparisonOp::LessEq => c_val <= val_content,
                    ComparisonOp::Eq => c_val == val_content,
                    ComparisonOp::NotEq => c_val != val_content,
                }
            } else {
                false
            }
        }
        _ => false,
    };
    Ok(result)
}

mod tests {
    use crate::{ast::Atom, parse::parse};

    use super::*;

    #[test]
    fn test_basic_eval() {
        let context = HashMap::from([
            ("a", Atom::Number(3)),
            ("b", Atom::String("demo".to_string())),
        ]);

        assert_eq!(eval(&parse("a < 4").unwrap().1, &context).unwrap(), true);
        assert_eq!(eval(&parse("a < 3.3").unwrap().1, &context).unwrap(), true);
        assert_eq!(
            eval(
                &parse("a < 3.1415").unwrap().1,
                &HashMap::from([("a", Atom::Float(3.0))])
            )
            .unwrap(),
            true
        );
        assert_eq!(eval(&parse("a>4").unwrap().1, &context).unwrap(), false);
        assert_eq!(eval(&parse("a<=4").unwrap().1, &context).unwrap(), true);
        assert_eq!(eval(&parse("a>=3").unwrap().1, &context).unwrap(), true);
        assert_eq!(eval(&parse("a!=4").unwrap().1, &context).unwrap(), true);
        assert_eq!(eval(&parse("a==4").unwrap().1, &context).unwrap(), false);
        assert_eq!(eval(&parse("a==3").unwrap().1, &context).unwrap(), true);
    }
}
