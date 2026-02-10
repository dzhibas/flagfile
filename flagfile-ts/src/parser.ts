import {
    Atom,
    AstNode,
    ComparisonOp,
    LogicOp,
    ArrayOp,
    MatchOp,
    FnCall,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
    atomVariable,
    atomDate,
    atomSemver,
    atomRegex,
} from './ast.js';

// ── Parser result type ─────────────────────────────────────────────

export type ParseResult<T> = { ok: true; rest: string; value: T } | { ok: false };

function ok<T>(rest: string, value: T): ParseResult<T> {
    return { ok: true, rest, value };
}

function fail<T>(): ParseResult<T> {
    return { ok: false };
}

/** Try multiple parsers, return first success. */
function alt<T>(...parsers: Array<() => ParseResult<T>>): ParseResult<T> {
    for (const p of parsers) {
        const r = p();
        if (r.ok) return r;
    }
    return fail();
}

// ── Whitespace helper ──────────────────────────────────────────────

function skipWs(i: string): string {
    let j = 0;
    while (j < i.length && (i[j] === ' ' || i[j] === '\t' || i[j] === '\n' || i[j] === '\r')) {
        j++;
    }
    return i.slice(j);
}

// ── Atom parsers ───────────────────────────────────────────────────

function parseDate(i: string): ParseResult<Atom> {
    const m = i.match(/^(\d{4})-(\d{2})-(\d{2})/);
    if (!m) return fail();
    const month = parseInt(m[2], 10);
    const day = parseInt(m[3], 10);
    if (month < 1 || month > 12 || day < 1 || day > 31) return fail();
    const dateStr = `${m[1]}-${m[2]}-${m[3]}`;
    return ok(i.slice(m[0].length), atomDate(dateStr));
}

function parseString(i: string): ParseResult<Atom> {
    if (i[0] === '"') {
        const end = i.indexOf('"', 1);
        if (end === -1) return fail();
        return ok(i.slice(end + 1), atomString(i.slice(1, end)));
    }
    if (i[0] === "'") {
        const end = i.indexOf("'", 1);
        if (end === -1) return fail();
        return ok(i.slice(end + 1), atomString(i.slice(1, end)));
    }
    return fail();
}

function parseBoolean(i: string): ParseResult<Atom> {
    const lower = i.slice(0, 5).toLowerCase();
    if (lower.startsWith('true')) {
        const after = i[4];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(4), atomBoolean(true));
    }
    if (lower.startsWith('false')) {
        const after = i[5];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(5), atomBoolean(false));
    }
    return fail();
}

function parseSemver(i: string): ParseResult<Atom> {
    const m = i.match(/^(\d+)\.(\d+)\.(\d+)/);
    if (!m) return fail();
    const rest = i.slice(m[0].length);
    return ok(rest, atomSemver(
        parseInt(m[1], 10),
        parseInt(m[2], 10),
        parseInt(m[3], 10),
    ));
}

function parseFloat_(i: string): ParseResult<Atom> {
    const m = i.match(/^([+-]?(?:\d+\.\d*|\.\d+)(?:[eE][+-]?\d+)?)/);
    if (!m) return fail();
    return ok(i.slice(m[0].length), atomFloat(parseFloat(m[0])));
}

function parseNumber(i: string): ParseResult<Atom> {
    const m = i.match(/^(-?\d+)/);
    if (!m) return fail();
    const rest = i.slice(m[0].length);
    if (rest[0] === '.') return fail();
    return ok(rest, atomNumber(parseInt(m[0], 10)));
}

function parseVariable(i: string): ParseResult<Atom> {
    const m = i.match(/^([a-zA-Z_][a-zA-Z0-9_]*)/);
    if (!m) return fail();
    return ok(i.slice(m[0].length), atomVariable(m[1]));
}

export function parseAtom(i: string): ParseResult<Atom> {
    return alt(
        () => parseDate(i),
        () => parseString(i),
        () => parseBoolean(i),
        () => parseSemver(i),
        () => parseFloat_(i),
        () => parseNumber(i),
        () => parseVariable(i),
    );
}

// ── Operator parsers ───────────────────────────────────────────────

function parseComparisonOp(i: string): ParseResult<ComparisonOp> {
    if (i.startsWith('!=')) return ok(i.slice(2), ComparisonOp.NotEq);
    if (i.startsWith('<>')) return ok(i.slice(2), ComparisonOp.NotEq);
    if (i.startsWith('==')) return ok(i.slice(2), ComparisonOp.Eq);
    if (i.startsWith('<=')) return ok(i.slice(2), ComparisonOp.LessEq);
    if (i.startsWith('>=')) return ok(i.slice(2), ComparisonOp.MoreEq);
    if (i.startsWith('='))  return ok(i.slice(1), ComparisonOp.Eq);
    if (i.startsWith('<'))  return ok(i.slice(1), ComparisonOp.Less);
    if (i.startsWith('>'))  return ok(i.slice(1), ComparisonOp.More);
    return fail();
}

function parseLogicOp(i: string): ParseResult<LogicOp> {
    if (i.startsWith('&&')) return ok(i.slice(2), LogicOp.And);
    if (i.startsWith('||')) return ok(i.slice(2), LogicOp.Or);

    const lower = i.toLowerCase();
    if (lower.startsWith('and')) {
        const after = i[3];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(3), LogicOp.And);
    }
    if (lower.startsWith('or')) {
        const after = i[2];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(2), LogicOp.Or);
    }
    return fail();
}

function parseArrayOp(i: string): ParseResult<ArrayOp> {
    const lower = i.toLowerCase();
    if (lower.startsWith('not in')) {
        const after = i[6];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(6), ArrayOp.NotIn);
    }
    if (lower.startsWith('in')) {
        const after = i[2];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(2), ArrayOp.In);
    }
    return fail();
}

// ── Function name parser ───────────────────────────────────────────

function parseFnName(i: string): ParseResult<FnCall> {
    const lower = i.toLowerCase();
    if (lower.startsWith('upper')) {
        const after = i[5];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(5), FnCall.Upper);
    }
    if (lower.startsWith('lower')) {
        const after = i[5];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(5), FnCall.Lower);
    }
    return fail();
}

// ── List parser: (a, b, c) ─────────────────────────────────────────

function parseList(i: string): ParseResult<AstNode> {
    if (i[0] !== '(') return fail();
    let rest = i.slice(1);
    const items: Atom[] = [];

    rest = skipWs(rest);
    const first = parseAtom(rest);
    if (first.ok) {
        items.push(first.value);
        rest = first.rest;
        while (true) {
            rest = skipWs(rest);
            if (rest[0] !== ',') break;
            rest = skipWs(rest.slice(1));
            const next = parseAtom(rest);
            if (!next.ok) break;
            items.push(next.value);
            rest = next.rest;
        }
    }

    rest = skipWs(rest);
    if (rest[0] !== ')') return fail();
    return ok(rest.slice(1), { type: 'List', items });
}

// ── Variable node parsers ──────────────────────────────────────────

function parseVariableNode(i: string): ParseResult<AstNode> {
    const r = parseVariable(i);
    if (!r.ok) return fail();
    return ok(r.rest, { type: 'Variable', atom: r.value });
}

function parseNullaryFunction(i: string): ParseResult<AstNode> {
    const lower = i.toLowerCase();
    if (lower.startsWith('now()')) {
        return ok(i.slice(5), {
            type: 'Function',
            fn: FnCall.Now,
            arg: { type: 'Void' },
        });
    }
    return fail();
}

function parseVariableNodeModifier(i: string): ParseResult<AstNode> {
    const fnR = parseFnName(skipWs(i));
    if (!fnR.ok) return fail();
    let rest = skipWs(fnR.rest);
    if (rest[0] !== '(') return fail();
    rest = skipWs(rest.slice(1));
    const varR = parseVariableNode(rest);
    if (!varR.ok) return fail();
    rest = skipWs(varR.rest);
    if (rest[0] !== ')') return fail();
    return ok(rest.slice(1), {
        type: 'Function',
        fn: fnR.value,
        arg: varR.value,
    });
}

function parseVariableNodeOrModified(i: string): ParseResult<AstNode> {
    return alt(
        () => parseNullaryFunction(i),
        () => parseVariableNodeModifier(i),
        () => parseVariableNode(i),
    );
}

// ── Constant node ──────────────────────────────────────────────────

function parseConstant(i: string): ParseResult<AstNode> {
    const r = parseAtom(i);
    if (!r.ok) return fail();
    return ok(r.rest, { type: 'Constant', atom: r.value });
}

// ── Comparison expression: var op constant ─────────────────────────

function parseCompareExpr(i: string): ParseResult<AstNode> {
    const varR = parseVariableNodeOrModified(i);
    if (!varR.ok) return fail();
    const opR = parseComparisonOp(skipWs(varR.rest));
    if (!opR.ok) return fail();
    const valR = parseConstant(skipWs(opR.rest));
    if (!valR.ok) return fail();
    return ok(valR.rest, {
        type: 'Compare',
        left: varR.value,
        op: opR.value,
        right: valR.value,
    });
}

// ── Array expression: var in/not in (list) ─────────────────────────

function parseArrayExpr(i: string): ParseResult<AstNode> {
    const varR = parseVariableNodeOrModified(i);
    if (!varR.ok) return fail();
    const opR = parseArrayOp(skipWs(varR.rest));
    if (!opR.ok) return fail();
    const listR = parseList(skipWs(opR.rest));
    if (!listR.ok) return fail();
    return ok(listR.rest, {
        type: 'Array',
        left: varR.value,
        op: opR.value,
        right: listR.value,
    });
}

// ── Regex literal parser ───────────────────────────────────────────

function parseRegexLiteral(i: string): ParseResult<Atom> {
    if (i[0] !== '/') return fail();
    const end = i.indexOf('/', 1);
    if (end === -1) return fail();
    const pattern = i.slice(1, end);
    return ok(i.slice(end + 1), atomRegex(pattern));
}

// ── Match operators: ~ and !~ ──────────────────────────────────────

function parseMatchOp(i: string): ParseResult<MatchOp> {
    if (i.startsWith('!^~')) return ok(i.slice(3), MatchOp.NotStartsWith);
    if (i.startsWith('!~$')) return ok(i.slice(3), MatchOp.NotEndsWith);
    if (i.startsWith('!~')) return ok(i.slice(2), MatchOp.NotContains);
    if (i.startsWith('^~')) return ok(i.slice(2), MatchOp.StartsWith);
    if (i.startsWith('~$')) return ok(i.slice(2), MatchOp.EndsWith);
    if (i.startsWith('~')) return ok(i.slice(1), MatchOp.Contains);
    return fail();
}

function parseMatchRhs(i: string): ParseResult<AstNode> {
    return alt(
        () => {
            const r = parseRegexLiteral(i);
            if (!r.ok) return fail<AstNode>();
            return ok(r.rest, { type: 'Constant' as const, atom: r.value });
        },
        () => parseConstant(i),
    );
}

function parseMatchExpr(i: string): ParseResult<AstNode> {
    const varR = parseVariableNodeOrModified(i);
    if (!varR.ok) return fail();
    const opR = parseMatchOp(skipWs(varR.rest));
    if (!opR.ok) return fail();
    const rhsR = parseMatchRhs(skipWs(opR.rest));
    if (!rhsR.ok) return fail();
    return ok(rhsR.rest, {
        type: 'Match',
        left: varR.value,
        op: opR.value,
        right: rhsR.value,
    });
}

// ── Compare or Array expr ──────────────────────────────────────────

function parseCompareOrArrayExpr(i: string): ParseResult<AstNode> {
    return alt(
        () => parseArrayExpr(i),
        () => parseMatchExpr(i),
        () => parseCompareExpr(i),
    );
}

// ── Parenthesized / scoped expression ──────────────────────────────

function parseParenthesizedExpr(i: string): ParseResult<AstNode> {
    let rest = i;
    let negate = false;

    const lower = rest.toLowerCase();
    if (lower.startsWith('not ') || lower.startsWith('not\t') || lower.startsWith('not(')) {
        negate = true;
        rest = skipWs(rest.slice(3));
    } else if (rest[0] === '!') {
        negate = true;
        rest = skipWs(rest.slice(1));
    }

    rest = skipWs(rest);
    if (rest[0] !== '(') return fail();
    rest = skipWs(rest.slice(1));

    const exprR = parseExpr(rest);
    if (!exprR.ok) return fail();
    rest = skipWs(exprR.rest);

    if (rest[0] !== ')') return fail();
    return ok(rest.slice(1), {
        type: 'Scope',
        expr: exprR.value,
        negate,
    });
}

// ── Main expression parser ─────────────────────────────────────────

function parseExpr(input: string): ParseResult<AstNode> {
    let headR = alt<AstNode>(
        () => parseParenthesizedExpr(input),
        () => parseLogicExprOnce(input),
        () => parseCompareOrArrayExpr(input),
        () => parseConstant(input),
    );
    if (!headR.ok) return fail();

    let head = headR.value;
    let rest = headR.rest;

    while (true) {
        const opR = parseLogicOp(skipWs(rest));
        if (!opR.ok) break;
        const rhsR = alt<AstNode>(
            () => parseCompareOrArrayExpr(skipWs(opR.rest)),
            () => parseParenthesizedExpr(skipWs(opR.rest)),
        );
        if (!rhsR.ok) break;
        head = { type: 'Logic', left: head, op: opR.value, right: rhsR.value };
        rest = rhsR.rest;
    }

    return ok(rest, head);
}

function parseLogicExprOnce(i: string): ParseResult<AstNode> {
    const leftR = alt<AstNode>(
        () => parseCompareOrArrayExpr(i),
        () => parseParenthesizedExpr(i),
    );
    if (!leftR.ok) return fail();
    const opR = parseLogicOp(skipWs(leftR.rest));
    if (!opR.ok) return fail();
    const rightR = alt<AstNode>(
        () => parseCompareOrArrayExpr(skipWs(opR.rest)),
        () => parseParenthesizedExpr(skipWs(opR.rest)),
    );
    if (!rightR.ok) return fail();
    return ok(rightR.rest, {
        type: 'Logic',
        left: leftR.value,
        op: opR.value,
        right: rightR.value,
    });
}

// ── Public API ─────────────────────────────────────────────────────

export function parse(i: string): ParseResult<AstNode> {
    const trimmed = skipWs(i);
    const r = parseExpr(trimmed);
    if (!r.ok) return parseParenthesizedExpr(trimmed);
    return r;
}
