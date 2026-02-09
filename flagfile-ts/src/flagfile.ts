import {
    AstNode,
    FlagReturn,
    FlagValue,
    Rule,
} from './ast.js';
import { parse, parseAtom, ParseResult } from './parser.js';

// ── Helpers ────────────────────────────────────────────────────────

function skipWs(i: string): string {
    let j = 0;
    while (j < i.length && (i[j] === ' ' || i[j] === '\t' || i[j] === '\n' || i[j] === '\r')) {
        j++;
    }
    return i.slice(j);
}

function fail<T>(): ParseResult<T> {
    return { ok: false };
}

function ok<T>(rest: string, value: T): ParseResult<T> {
    return { ok: true, rest, value };
}

// ── Comment parsers ────────────────────────────────────────────────

function skipLineComment(i: string): string {
    const trimmed = skipWs(i);
    if (trimmed.startsWith('//')) {
        const eol = trimmed.indexOf('\n');
        if (eol === -1) return '';
        return trimmed.slice(eol + 1);
    }
    return i;
}

function skipBlockComment(i: string): string {
    const trimmed = skipWs(i);
    if (trimmed.startsWith('/*')) {
        const end = trimmed.indexOf('*/');
        if (end === -1) return '';
        return trimmed.slice(end + 2);
    }
    return i;
}

function skipComments(i: string): string {
    let prev = '';
    let cur = i;
    while (cur !== prev) {
        prev = cur;
        cur = skipWs(cur);
        cur = skipLineComment(cur);
        cur = skipBlockComment(cur);
    }
    return skipWs(cur);
}

// ── Flag name parser ───────────────────────────────────────────────

function parseFlagName(i: string): ParseResult<string> {
    const m = i.match(/^(FF[-_][a-zA-Z0-9_-]*)/);
    if (!m) return fail();
    return ok(i.slice(m[0].length), m[1]);
}

// ── Return value parsers ───────────────────────────────────────────

function parseBoolReturn(i: string): ParseResult<FlagReturn> {
    const lower = i.slice(0, 5).toLowerCase();
    if (lower.startsWith('true')) {
        const after = i[4];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(4), { type: 'OnOff', value: true });
    }
    if (lower.startsWith('false')) {
        const after = i[5];
        if (after && /[a-zA-Z0-9_]/.test(after)) return fail();
        return ok(i.slice(5), { type: 'OnOff', value: false });
    }
    return fail();
}

function parseJsonReturn(i: string): ParseResult<FlagReturn> {
    const trimmed = skipWs(i);
    if (!trimmed.startsWith('json(')) return fail();
    let rest = trimmed.slice(5);

    // Find the matching closing paren, accounting for nested braces
    let depth = 0;
    let j = 0;
    let foundEnd = false;
    for (; j < rest.length; j++) {
        if (rest[j] === '{') depth++;
        else if (rest[j] === '}') depth--;
        else if (rest[j] === ')' && depth === 0) {
            foundEnd = true;
            break;
        }
    }
    if (!foundEnd) return fail();

    const jsonStr = rest.slice(0, j);
    rest = rest.slice(j + 1);

    try {
        const value = JSON.parse(jsonStr);
        return ok(rest, { type: 'Json', value });
    } catch {
        return fail();
    }
}

function parseStringReturn(i: string): ParseResult<FlagReturn> {
    if (i[0] === '"') {
        const end = i.indexOf('"', 1);
        if (end === -1) return fail();
        return ok(i.slice(end + 1), { type: 'Str', value: i.slice(1, end) });
    }
    if (i[0] === "'") {
        const end = i.indexOf("'", 1);
        if (end === -1) return fail();
        return ok(i.slice(end + 1), { type: 'Str', value: i.slice(1, end) });
    }
    return fail();
}

function parseIntegerReturn(i: string): ParseResult<FlagReturn> {
    const m = i.match(/^(-?\d+)/);
    if (!m) return fail();
    // Make sure it's not followed by a dot (would be float)
    const rest = i.slice(m[0].length);
    if (rest[0] === '.') return fail();
    return ok(rest, { type: 'Integer', value: parseInt(m[0], 10) });
}

function parseReturnVal(i: string): ParseResult<FlagReturn> {
    const trimmed = skipWs(i);
    // Try in order: bool, json, string, integer
    let r: ParseResult<FlagReturn>;
    r = parseBoolReturn(trimmed);
    if (r.ok) return r;
    r = parseJsonReturn(trimmed);
    if (r.ok) return r;
    r = parseStringReturn(trimmed);
    if (r.ok) return r;
    r = parseIntegerReturn(trimmed);
    if (r.ok) return r;
    return fail();
}

// ── Arrow parser ───────────────────────────────────────────────────

function parseArrow(i: string): ParseResult<null> {
    const trimmed = skipWs(i);
    if (trimmed.startsWith('->')) {
        return ok(trimmed.slice(2), null);
    }
    return fail();
}

// ── Short notation: FF-name -> value ───────────────────────────────

function parseAnonymousFunc(i: string): ParseResult<[string, Rule[]]> {
    const nameR = parseFlagName(skipWs(i));
    if (!nameR.ok) return fail();
    const arrowR = parseArrow(skipWs(nameR.rest));
    if (!arrowR.ok) return fail();
    const valR = parseReturnVal(arrowR.rest);
    if (!valR.ok) return fail();
    return ok(valR.rest, [nameR.value, [{ type: 'Value', value: valR.value }]]);
}

// ── Rule parsers ───────────────────────────────────────────────────

function parseRuleExpr(i: string): ParseResult<Rule> {
    const exprR = parse(i);
    if (!exprR.ok) return fail();
    const arrowR = parseArrow(skipWs(exprR.rest));
    if (!arrowR.ok) return fail();
    const valR = parseReturnVal(arrowR.rest);
    if (!valR.ok) return fail();
    return ok(valR.rest, {
        type: 'BoolExpressionValue',
        expr: exprR.value,
        value: valR.value,
    });
}

function parseRuleStatic(i: string): ParseResult<Rule> {
    const valR = parseReturnVal(i);
    if (!valR.ok) return fail();
    return ok(valR.rest, { type: 'Value', value: valR.value });
}

function parseRule(i: string): ParseResult<Rule> {
    let r: ParseResult<Rule>;
    r = parseRuleExpr(i);
    if (r.ok) return r;
    r = parseRuleStatic(i);
    if (r.ok) return r;
    return fail();
}

function parseRulesList(i: string): ParseResult<Rule[]> {
    const rules: Rule[] = [];
    let rest = i;

    while (true) {
        rest = skipComments(rest);
        if (rest.length === 0 || rest[0] === '}') break;
        const ruleR = parseRule(rest);
        if (!ruleR.ok) break;
        rules.push(ruleR.value);
        rest = ruleR.rest;
    }

    if (rules.length === 0) return fail();
    return ok(rest, rules);
}

// ── Block notation: FF-name { rules... } ───────────────────────────

function parseFunction(i: string): ParseResult<[string, Rule[]]> {
    const nameR = parseFlagName(skipWs(i));
    if (!nameR.ok) return fail();
    let rest = skipWs(nameR.rest);
    if (rest[0] !== '{') return fail();
    rest = skipWs(rest.slice(1));

    const rulesR = parseRulesList(rest);
    if (!rulesR.ok) return fail();
    rest = skipWs(rulesR.rest);

    if (rest[0] !== '}') return fail();
    rest = rest.slice(1);

    return ok(rest, [nameR.value, rulesR.value]);
}

// ── Main flagfile parser ───────────────────────────────────────────

export function parseFlagfile(i: string): ParseResult<FlagValue> {
    const flags: FlagValue = new Map();
    let rest = i;

    while (true) {
        rest = skipComments(rest);
        if (rest.length === 0) break;

        let r: ParseResult<[string, Rule[]]>;
        r = parseAnonymousFunc(rest);
        if (!r.ok) {
            r = parseFunction(rest);
        }
        if (!r.ok) break;

        const [name, rules] = r.value;
        flags.set(name, rules);
        rest = r.rest;
    }

    return ok(skipWs(rest), flags);
}
