use std::collections::HashMap;

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FnCall, LogicOp};

pub fn eval<'a>(expr: &AstNode, context: &HashMap<&str, Atom>) -> Result<bool, &'a str> {
    let mut inner_context = context.clone();
    let mut result = false;
    result = match expr {
        // true || false
        AstNode::Constant(var) => {
            let mut result = false;
            if let Atom::Boolean(v) = var {
                result = *v;
            }
            if let Atom::Variable(v) = var {
                let context_val = inner_context.get(v.as_str());
                if let Some(Atom::Boolean(inner)) = context_val {
                    result = *inner;
                }
            }
            result
        }
        // a == 3
        // a < 3
        AstNode::Compare(var, op, val) => {
            let context_val = inner_context.get(var.as_str().unwrap());
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
                    let var_value = inner_context.get(&var.as_str());
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
        AstNode::Logic(expr1, op, expr2) => {
            let expr1_eval = eval(expr1, &inner_context).unwrap();
            let expr2_eval = eval(expr2, &inner_context).unwrap();
            match op {
                LogicOp::And => expr1_eval && expr2_eval,
                LogicOp::Or => expr1_eval || expr2_eval,
            }
        }
        AstNode::Function(func, variable) => match func {
            FnCall::Upper => {
                if let AstNode::Variable(Atom::Variable(var)) = variable.as_ref() {
                    if let Some(v) = inner_context.get_mut(var.as_str()) {
                        let out = match v {
                            Atom::String(val) => Atom::String(val.to_uppercase()),
                            Atom::Number(val) => Atom::Number(*val),
                            Atom::Float(val) => Atom::Float(*val),
                            Atom::Boolean(val) => Atom::Boolean(*val),
                            Atom::Variable(val) => Atom::Variable(val.to_uppercase()),
                            Atom::Date(val) => Atom::Date(*val),
                            Atom::DateTime(val) => Atom::DateTime(val.to_string()),
                        };
                        *v = out;
                    }
                }
                false
            }
            FnCall::Lower => {
                if let AstNode::Variable(Atom::Variable(var)) = variable.as_ref() {
                    if let Some(v) = inner_context.get_mut(var.as_str()) {
                        let out = match v {
                            Atom::String(val) => Atom::String(val.to_lowercase()),
                            Atom::Number(val) => Atom::Number(*val),
                            Atom::Float(val) => Atom::Float(*val),
                            Atom::Boolean(val) => Atom::Boolean(*val),
                            Atom::Variable(val) => Atom::Variable(val.to_lowercase()),
                            Atom::Date(val) => Atom::Date(*val),
                            Atom::DateTime(val) => Atom::DateTime(val.to_string()),
                        };
                        *v = out;
                    }
                }
                false
            }
        },
        AstNode::Scope { expr, negate } => {
            let res = eval(expr, &inner_context).unwrap();
            match negate {
                true => !res,
                false => res,
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
    fn logic_test() {
        let (i, expr) = parse("x=1 and y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(1)), ("y", Atom::Number(2))])
            )
            .unwrap()
        );

        let (i, expr) = parse("x=1 || y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(12)), ("y", Atom::Number(2))])
            )
            .unwrap()
        );

        let (i, expr) = parse("countryCode==LT && city='Palanga'").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([
                    ("countryCode", Atom::String("LT".to_string())),
                    ("city", Atom::String("Palanga".to_string()))
                ])
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_function_calls() {
        // let (i, expr) = parse("lower(countryCode)==LT && city='Palanga'").unwrap();
        // assert_eq!(
        //     true,
        //     eval(
        //         &expr,
        //         &HashMap::from([
        //             ("countryCode", Atom::String("LT".to_string())),
        //             ("city", Atom::String("Palanga".to_string()))
        //         ])
        //     )
        //     .unwrap()
        // );
    }

    #[test]
    fn simple_scope_test() {
        let (i, expr) = parse("!(country=LT)").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("LT".to_string()))])
            )
            .unwrap()
        );

        // scope inside scope
        let (i, expr) = parse("(not (country == Lithuania))").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("Lithuania".to_string()))])
            )
            .unwrap()
        );

        let (i, expr) = parse("((country == Netherlands))").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("Netherlands".to_string()))])
            )
            .unwrap()
        );
    }

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
