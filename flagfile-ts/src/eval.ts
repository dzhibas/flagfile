import { createHash } from 'crypto';
import {
    Atom,
    AstNode,
    ComparisonOp,
    ArrayOp,
    LogicOp,
    MatchOp,
    FnCall,
    Segments,
    atomEquals,
    atomCompare,
    atomToString,
    atomDateTime,
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
                const now = new Date();
                const yyyy = now.getFullYear();
                const mm = String(now.getMonth() + 1).padStart(2, '0');
                const dd = String(now.getDate()).padStart(2, '0');
                const hh = String(now.getHours()).padStart(2, '0');
                const min = String(now.getMinutes()).padStart(2, '0');
                const ss = String(now.getSeconds()).padStart(2, '0');
                return atomDateTime(`${yyyy}-${mm}-${dd}T${hh}:${min}:${ss}`);
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
        case 'Coalesce': {
            for (const arg of variable.args) {
                if (arg.type === 'Variable' && arg.atom.type === 'Variable') {
                    const val = context[arg.atom.value];
                    if (val !== undefined) return val;
                } else if (arg.type === 'Constant') {
                    return arg.atom;
                }
            }
            return null;
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

export function evaluate(expr: AstNode, context: Context, flagName?: string, segments?: Segments): boolean {
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
                switch (expr.op) {
                    case MatchOp.StartsWith:
                    case MatchOp.NotStartsWith:
                        matched = haystack.startsWith(needle);
                        break;
                    case MatchOp.EndsWith:
                    case MatchOp.NotEndsWith:
                        matched = haystack.endsWith(needle);
                        break;
                    default:
                        matched = haystack.includes(needle);
                        break;
                }
            }
            switch (expr.op) {
                case MatchOp.Contains:
                case MatchOp.StartsWith:
                case MatchOp.EndsWith:
                    return matched;
                case MatchOp.NotContains:
                case MatchOp.NotStartsWith:
                case MatchOp.NotEndsWith:
                    return !matched;
            }
        }

        case 'Array': {
            // Case 1: variable in (literal_list)
            if (expr.right.type === 'List') {
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
            // Case 2: "literal" in variable (variable resolves to List in context)
            else {
                let searchValue: Atom | null;
                if (expr.left.type === 'Constant' && expr.left.atom.type !== 'Variable') {
                    searchValue = expr.left.atom;
                } else {
                    searchValue = getVariableValueFromContext(expr.left, context);
                }
                const listValue = getVariableValueFromContext(expr.right, context);
                if (searchValue === null || listValue === null || listValue.type !== 'List') {
                    return false;
                }
                const found = listValue.items.some(item => atomEquals(searchValue!, item));
                switch (expr.op) {
                    case ArrayOp.In: return found;
                    case ArrayOp.NotIn: return !found;
                }
                return false;
            }
        }

        case 'Logic': {
            const left = evaluate(expr.left, context, flagName, segments);
            const right = evaluate(expr.right, context, flagName, segments);
            switch (expr.op) {
                case LogicOp.And: return left && right;
                case LogicOp.Or:  return left || right;
            }
            return false;
        }

        case 'Scope': {
            const result = evaluate(expr.expr, context, flagName, segments);
            return expr.negate ? !result : result;
        }

        case 'Segment': {
            if (!segments) return false;
            const segExpr = segments.get(expr.name);
            if (!segExpr) return false;
            return evaluate(segExpr, context, flagName, segments);
        }

        case 'Percentage': {
            const inner = getVariableValueFromContext(expr.field, context);
            if (inner === null) return false;
            const bucketKey = atomToString(inner);
            const flag = flagName ?? 'unknown';
            const input = expr.salt
                ? `${flag}.${expr.salt}.${bucketKey}`
                : `${flag}.${bucketKey}`;
            const hex = createHash('sha1').update(input).digest('hex');
            const substr = hex.slice(0, 15);
            const value = parseInt(substr, 16);
            const bucket = value % 100000;
            const threshold = expr.rate * 1000;
            return bucket < threshold;
        }

        default:
            return false;
    }
}
