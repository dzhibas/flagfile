import {
    Atom,
    AstNode,
    ComparisonOp,
    ArrayOp,
    LogicOp,
    MatchOp,
    FnCall,
    atomEquals,
    atomCompare,
    atomToString,
    atomDate,
    atomString,
} from './ast.js';

// ── Context type ──────────────────────────────────────────────────

export type Context = Record<string, Atom>;

// ── Variable resolution ───────────────────────────────────────────

function getVariableValueFromContext(
    variable: AstNode,
    context: Context,
): Atom | null {
    switch (variable.type) {
        case 'Variable': {
            const name =
                variable.atom.type === 'Variable' ? variable.atom.value : null;
            if (name === null) return null;
            return context[name] ?? null;
        }
        case 'Constant': {
            if (variable.atom.type === 'Variable') {
                return context[variable.atom.value] ?? null;
            }
            return null;
        }
        case 'Function': {
            if (variable.fn === FnCall.Now) {
                const today = new Date();
                const yyyy = today.getFullYear();
                const mm = String(today.getMonth() + 1).padStart(2, '0');
                const dd = String(today.getDate()).padStart(2, '0');
                return atomDate(`${yyyy}-${mm}-${dd}`);
            }
            const inner = getVariableValueFromContext(variable.arg, context);
            if (inner === null) return null;
            const s = atomToString(inner);
            switch (variable.fn) {
                case FnCall.Upper:
                    return atomString(s.toUpperCase());
                case FnCall.Lower:
                    return atomString(s.toLowerCase());
                default:
                    return null;
            }
        }
        default:
            return null;
    }
}

// ── Comparison helper ─────────────────────────────────────────────

function evalComparison(contextVal: Atom, op: ComparisonOp, rhs: Atom): boolean {
    switch (op) {
        case ComparisonOp.Eq:
            return atomEquals(contextVal, rhs);
        case ComparisonOp.NotEq:
            return !atomEquals(contextVal, rhs);
        case ComparisonOp.More:
        case ComparisonOp.MoreEq:
        case ComparisonOp.Less:
        case ComparisonOp.LessEq: {
            const cmp = atomCompare(contextVal, rhs);
            if (cmp === null) return false;
            switch (op) {
                case ComparisonOp.More:   return cmp > 0;
                case ComparisonOp.MoreEq: return cmp >= 0;
                case ComparisonOp.Less:   return cmp < 0;
                case ComparisonOp.LessEq: return cmp <= 0;
            }
        }
    }
}

// ── Main evaluator ────────────────────────────────────────────────

export function evaluate(expr: AstNode, context: Context): boolean {
    switch (expr.type) {
        case 'Constant': {
            if (expr.atom.type === 'Boolean') {
                return expr.atom.value;
            }
            if (expr.atom.type === 'Variable') {
                const val = getVariableValueFromContext(expr, context);
                if (val !== null && val.type === 'Boolean') {
                    return val.value;
                }
            }
            return false;
        }

        case 'Compare': {
            const contextVal = getVariableValueFromContext(expr.left, context);
            const rhsAtom =
                expr.right.type === 'Constant' ? expr.right.atom : null;
            if (contextVal === null || rhsAtom === null) return false;
            return evalComparison(contextVal, expr.op, rhsAtom);
        }

        case 'Match': {
            const contextVal = getVariableValueFromContext(expr.left, context);
            if (contextVal === null) return false;
            const haystack = atomToString(contextVal);
            const rhsAtom = expr.right.type === 'Constant' ? expr.right.atom : null;
            if (rhsAtom === null) return false;
            let matched: boolean;
            if (rhsAtom.type === 'Regex') {
                try {
                    matched = new RegExp(rhsAtom.value).test(haystack);
                } catch {
                    matched = false;
                }
            } else {
                const needle = atomToString(rhsAtom);
                matched = haystack.includes(needle);
            }
            return expr.op === MatchOp.Contains ? matched : !matched;
        }

        case 'Array': {
            if (expr.right.type !== 'List') return false;
            const varValue = getVariableValueFromContext(expr.left, context);
            if (varValue === null) return false;
            const list = expr.right.items;

            switch (expr.op) {
                case ArrayOp.In: {
                    for (const item of list) {
                        if (atomEquals(varValue, item)) return true;
                    }
                    return false;
                }
                case ArrayOp.NotIn: {
                    for (const item of list) {
                        if (atomEquals(varValue, item)) return false;
                    }
                    return true;
                }
            }
            return false;
        }

        case 'Logic': {
            const left = evaluate(expr.left, context);
            const right = evaluate(expr.right, context);
            switch (expr.op) {
                case LogicOp.And: return left && right;
                case LogicOp.Or:  return left || right;
            }
            return false;
        }

        case 'Scope': {
            const result = evaluate(expr.expr, context);
            return expr.negate ? !result : result;
        }

        default:
            return false;
    }
}
