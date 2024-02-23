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
                dbg!(c_val, val_content);
                let res = match op {
                    ComparisonOp::More => c_val > val_content,
                    ComparisonOp::MoreEq => c_val >= val_content,
                    _ => false,
                };
                dbg!(context_val);
                res
            } else {
                false
            }
        }
        _ => false,
    };
    dbg!(expr);
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
        let (i, expr) = parse("a < 4 ").unwrap();

        let res = eval(&expr, &context).unwrap();
        assert_eq!(res, false);
    }
}
