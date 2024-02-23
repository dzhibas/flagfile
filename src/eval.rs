use std::collections::HashMap;

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FnCall, LogicOp};

pub type Context<'a> = HashMap<&'a str, Atom>;

fn get_variable_value_from_context<'a>(
    variable: &'a AstNode,
    context: &'a Context,
) -> Option<Atom> {
    let res = match variable {
        AstNode::Variable(Atom::Variable(v)) => context.get(v.as_str()),
        AstNode::Constant(Atom::Variable(v)) => context.get(v.as_str()),
        AstNode::Function(op, v) => {
            let value = get_variable_value_from_context(v, context);
            if let Some(v) = value {
                let vv = match op {
                    FnCall::Upper => Atom::String(v.to_string().to_uppercase()),
                    FnCall::Lower => Atom::String(v.to_string().to_lowercase()),
                };
                return Some(vv);
            }
            None
        }
        _ => None,
    };
    res.cloned()
}

pub fn eval<'a>(expr: &AstNode, context: &Context) -> Result<bool, &'a str> {
    let result = match expr {
        // true || false
        AstNode::Constant(var) => {
            let mut result = false;
            if let Atom::Boolean(v) = var {
                result = *v;
            }
            if let Atom::Variable(_v) = var {
                let context_val = get_variable_value_from_context(expr, context);
                if let Some(Atom::Boolean(inner)) = context_val {
                    result = inner;
                }
            }
            result
        }
        // a == 3
        // a < 3
        AstNode::Compare(var, op, val) => {
            let context_val = get_variable_value_from_context(var, context);
            let val_content = match val.as_ref() {
                AstNode::Constant(a) => Some(a),
                _ => None,
            }
            .unwrap();

            if let Some(c_val) = &context_val {
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
        AstNode::Array(var_expr, op, list) => {
            let mut result = false;
            if let AstNode::List(vec_list) = list.as_ref() {
                let var_value = get_variable_value_from_context(var_expr, context);
                if let Some(search_value) = &var_value {
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
            result
        }
        AstNode::Logic(expr1, op, expr2) => {
            let expr1_eval = eval(expr1, context).unwrap();
            let expr2_eval = eval(expr2, context).unwrap();
            match op {
                LogicOp::And => expr1_eval && expr2_eval,
                LogicOp::Or => expr1_eval || expr2_eval,
            }
        }
        AstNode::Scope { expr, negate } => {
            let res = eval(expr, context).unwrap();
            match negate {
                true => !res,
                false => res,
            }
        }
        _ => false,
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::{ast::Atom, parse::parse};

    use super::*;

    #[test]
    fn logic_test() {
        let (_i, expr) = parse("x=1 and y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(1)), ("y", Atom::Number(2))])
            )
            .unwrap()
        );

        let (_i, expr) = parse("x=1 || y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(12)), ("y", Atom::Number(2))])
            )
            .unwrap()
        );

        let (_i, expr) = parse("countryCode==LT && city='Palanga'").unwrap();
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
        let (_i, expr) = parse("lower(countryCode)==lt && upper(city)='PALANGA'").unwrap();
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
    fn simple_scope_test() {
        let (_i, expr) = parse("!(country=LT)").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("LT".to_string()))])
            )
            .unwrap()
        );

        // scope inside scope
        let (_i, expr) = parse("(not (country == Lithuania))").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("Lithuania".to_string()))])
            )
            .unwrap()
        );

        let (_i, expr) = parse("((lower(country) == netherlands))").unwrap();
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
        let (_i, expr) = parse("y in ('one', 'two', 'tree')").unwrap();
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
    fn testing_date_comparison_evaluation() {
        let (_i, expr) = parse("created > 2024-02-02 and created <= 2024-02-13").unwrap();
        assert_eq!(
            true,
            eval(&expr, &HashMap::from([("created", "2024-02-12".into())])).unwrap()
        );

        assert_eq!(
            false,
            eval(
                &parse("created < 2024-02-02").unwrap().1,
                &HashMap::from([("created", "2024-02-02".into())])
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_logical_expression() {
        assert_eq!(
            true,
            eval(
                &parse("a=b and (c=d or e=f)").unwrap().1,
                &HashMap::from([
                    ("a", "b".into()),
                    ("c", "non-exiting".into()),
                    ("e", "f".into())
                ])
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("a=b and (c=d or e=f)").unwrap().1,
                &HashMap::from([("a", "b".into()), ("c", "d".into()), ("e", "fnon".into())])
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([
                    ("a", "non".into()),
                    ("c", "non-exiting".into()),
                    ("e", "f".into())
                ])
            )
            .unwrap()
        );

        assert_eq!(
            true,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([("a", "non".into()), ("c", "non".into()), ("e", "f".into())])
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([("a", "non".into()), ("c", "d".into()), ("e", "non".into())])
            )
            .unwrap()
        );
    }
}
