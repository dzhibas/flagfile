use std::collections::HashMap;

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp};

pub fn eval<'a>(expr: &AstNode, context: &HashMap<&str, Atom>) -> Result<bool, &'a str> {
    let mut result = false;
    result = match expr {
        // true || false
        AstNode::Constant(var) => {
            let mut result = false;
            if let Atom::Boolean(v) = var {
                result = *v;
            }
            if let Atom::Variable(v) = var {
                let context_val = context.get(v.as_str());
                if let Some(Atom::Boolean(inner)) = context_val {
                    result = *inner;
                }
            }
            result
        }
        // a == 3
        // a < 3
        AstNode::Compare(var, op, val) => {
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
        // x in (1, 2, 3)
        AstNode::Array(var, op, list) => {
            let mut result = false;
            if let AstNode::List(vec_list) = list.as_ref() {
                if let AstNode::Variable(Atom::Variable(var)) = var.as_ref() {
                    let var_value = context.get(&var.as_str());
                    if let Some(search_value) = var_value {
                        match op {
                            ArrayOp::In => {
                                // check if this value is in the list
                                for i in vec_list.iter() {
                                    if search_value == i {
                                        result = true;
                                        break;
                                    }
                                }
                            }
                            ArrayOp::NotIn => {
                                // a not in (c,d)
                                let mut found = false;
                                for i in vec_list.iter() {
                                    if search_value == i {
                                        found = true;
                                    }
                                }
                                result = !found;
                            }
                        }
                    }
                }
            }
            result
        }
        _ => false,
    };
    Ok(result)
}

mod tests {
    use crate::{ast::Atom, parse::parse};

    use super::*;

    #[test]
    fn array_eval_test() {
        let context = HashMap::from([
            ("x", Atom::Number(10)),
            ("y", Atom::String("tree".to_string())),
        ]);
        let (i, expr) = parse("y in ('one', 'two', 'tree')").unwrap();
        let res = eval(&expr, &context).unwrap();
        assert_eq!(res, true);

        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("y", Atom::String("four".to_string())),])
            )
            .unwrap()
        );

        assert_eq!(
            true,
            eval(
                &parse("y not in ('one','two','tree')").unwrap().1,
                &HashMap::from([("y", Atom::String("four".to_string())),])
            )
            .unwrap()
        );
    }

    #[test]
    fn compare_variable_with_string_in_array_test() {
        assert_eq!(
            true,
            eval(
                &parse("y in (one,two,tree)").unwrap().1,
                &HashMap::from([("y", Atom::String("two".to_string())),])
            )
            .unwrap()
        );
    }

    #[test]
    fn test_comparison_expr_eval() {
        let context = HashMap::from([
            ("a", Atom::Number(3)),
            ("b", Atom::String("demo".to_string())),
        ]);

        assert_eq!(eval(&parse("a < 4").unwrap().1, &context).unwrap(), true);
        assert_eq!(eval(&parse("a < 3.3").unwrap().1, &context).unwrap(), true);
        assert_eq!(
            eval(
                &parse("a > 3.15").unwrap().1,
                &HashMap::from([("a", Atom::Float(3.14))])
            )
            .unwrap(),
            false
        );
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
    #[test]
    fn test_compare_string_expr_eval() {
        assert_eq!(
            eval(
                &parse("car!='Tesla'").unwrap().1,
                &HashMap::from([("car", Atom::String("BMW".into()))])
            )
            .unwrap(),
            true
        );
        assert_eq!(
            eval(
                &parse("car=='Tesla'").unwrap().1,
                &HashMap::from([("car", Atom::String("Tesla".into()))])
            )
            .unwrap(),
            true
        );
    }

    #[test]
    fn simple_constant_eval_test() {
        assert_eq!(
            false,
            eval(&parse("false").unwrap().1, &HashMap::from([])).unwrap()
        );
        assert_eq!(
            true,
            eval(&parse("TRUE").unwrap().1, &HashMap::from([])).unwrap()
        );
    }

    #[test]
    fn test_logic_expr_eval() {

        // assert_eq!(eval(&parse("a>4 and b<3").unwrap().1, &HashMap::from([])).unwrap(), true);
    }
}
