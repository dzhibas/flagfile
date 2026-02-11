import { describe, it, expect } from 'vitest';
import { parse } from './parser.js';
import { evaluate, Context } from './eval.js';
import {
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
    atomSemver,
    atomDate,
    AstNode,
} from './ast.js';

/** Helper: parse + evaluate, asserting parse success. */
function eval_(expr: string, ctx: Context): boolean {
    const r = parse(expr);
    if (!r.ok) throw new Error(`Parse failed for: ${expr}`);
    return evaluate(r.value, ctx);
}

// ── Constant booleans ─────────────────────────────────────────────

describe('constant booleans', () => {
    it('evaluates true/false constants', () => {
        expect(eval_('true', {})).toBe(true);
        expect(eval_('TRUE', {})).toBe(true);
        expect(eval_('false', {})).toBe(false);
        expect(eval_('FALSE', {})).toBe(false);
    });
});

// ── Comparison expressions ────────────────────────────────────────

describe('comparison expressions', () => {
    const ctx: Context = {
        a: atomNumber(3),
        b: atomString('demo'),
    };

    it('number comparisons', () => {
        expect(eval_('a < 4', ctx)).toBe(true);
        expect(eval_('a > 4', ctx)).toBe(false);
        expect(eval_('a <= 4', ctx)).toBe(true);
        expect(eval_('a >= 3', ctx)).toBe(true);
        expect(eval_('a != 4', ctx)).toBe(true);
        expect(eval_('a == 4', ctx)).toBe(false);
        expect(eval_('a == 3', ctx)).toBe(true);
    });

    it('number vs float comparisons', () => {
        expect(eval_('a < 3.3', ctx)).toBe(true);
        expect(eval_('a > 3.15', { a: atomFloat(3.14) })).toBe(false);
        expect(eval_('a < 3.1415', { a: atomFloat(3.0) })).toBe(true);
    });

    it('string comparisons', () => {
        expect(eval_("car != 'Tesla'", { car: atomString('BMW') })).toBe(true);
        expect(eval_("car == 'Tesla'", { car: atomString('Tesla') })).toBe(true);
    });

    it('missing variable returns false', () => {
        expect(eval_('x == 1', {})).toBe(false);
        expect(eval_('x > 1', {})).toBe(false);
    });
});

// ── Logic expressions ─────────────────────────────────────────────

describe('logic expressions', () => {
    it('and: both sides must be true', () => {
        expect(eval_('x=1 and y=2', {
            x: atomNumber(1),
            y: atomNumber(2),
        })).toBe(true);
        expect(eval_('x=1 and y=2', {
            x: atomNumber(1),
            y: atomNumber(99),
        })).toBe(false);
    });

    it('or: either side can be true', () => {
        expect(eval_('x=1 || y=2', {
            x: atomNumber(12),
            y: atomNumber(2),
        })).toBe(true);
        expect(eval_('x=1 || y=2', {
            x: atomNumber(12),
            y: atomNumber(99),
        })).toBe(false);
    });

    it('string logic with && operator', () => {
        expect(eval_("countryCode==LT && city='Palanga'", {
            countryCode: atomString('LT'),
            city: atomString('Palanga'),
        })).toBe(true);
    });

    it('complex logical expression: a=b and (c=d or e=f)', () => {
        expect(eval_('a=b and (c=d or e=f)', {
            a: atomString('b'),
            c: atomString('non-existing'),
            e: atomString('f'),
        })).toBe(true);
        expect(eval_('a=b and (c=d or e=f)', {
            a: atomString('b'),
            c: atomString('d'),
            e: atomString('f-non'),
        })).toBe(true);
    });

    it('operator precedence: a=b and c=d or e=f', () => {
        // Rust implementation: parses left-to-right, so (a=b and c=d) or e=f
        expect(eval_('a=b and c=d or e=f', {
            a: atomString('non'),
            c: atomString('non-existing'),
            e: atomString('f'),
        })).toBe(true);

        expect(eval_('a=b and c=d or e=f', {
            a: atomString('non'),
            c: atomString('d'),
            e: atomString('non'),
        })).toBe(false);
    });
});

// ── Function calls ────────────────────────────────────────────────

describe('function calls', () => {
    it('lower() and upper()', () => {
        expect(eval_("lower(countryCode)==lt && upper(city)='PALANGA'", {
            countryCode: atomString('LT'),
            city: atomString('Palanga'),
        })).toBe(true);
    });

    it('UPPER with comparison', () => {
        expect(eval_("UPPER(name) == 'HELLO'", {
            name: atomString('hello'),
        })).toBe(true);
    });

    it('LOWER with comparison', () => {
        expect(eval_("lower(name) == 'hello'", {
            name: atomString('HELLO'),
        })).toBe(true);
    });

    it('function on missing variable returns false', () => {
        expect(eval_("lower(missing) == 'test'", {})).toBe(false);
    });
});

// ── Scope and negation ────────────────────────────────────────────

describe('scope and negation', () => {
    it('negated scope: !(country=LT)', () => {
        expect(eval_('!(country=LT)', {
            country: atomString('LT'),
        })).toBe(false);
        expect(eval_('!(country=LT)', {
            country: atomString('US'),
        })).toBe(true);
    });

    it('nested scope: (not (country == Lithuania))', () => {
        expect(eval_('(not (country == Lithuania))', {
            country: atomString('Lithuania'),
        })).toBe(false);
    });

    it('double scope with lower(): ((lower(country) == netherlands))', () => {
        expect(eval_('((lower(country) == netherlands))', {
            country: atomString('Netherlands'),
        })).toBe(true);
    });

    it('nested scopes: (a=1 or b=2) and ((c=3 or d=4) and e=5)', () => {
        expect(eval_('(a=1 or b=2) and ((c=3 or d=4) and e=5)', {
            a: atomNumber(1),
            b: atomNumber(99),
            c: atomNumber(99),
            d: atomNumber(4),
            e: atomNumber(5),
        })).toBe(true);

        expect(eval_('(a=1 or b=2) and ((c=3 or d=4) and e=5)', {
            a: atomNumber(99),
            b: atomNumber(99),
            c: atomNumber(3),
            d: atomNumber(4),
            e: atomNumber(5),
        })).toBe(false);
    });
});

// ── Array membership (in / not in) ────────────────────────────────

describe('array membership', () => {
    it('in: string in list', () => {
        expect(eval_("y in ('one', 'two', 'tree')", {
            x: atomNumber(10),
            y: atomString('tree'),
        })).toBe(true);
        expect(eval_("y in ('one', 'two', 'tree')", {
            y: atomString('four'),
        })).toBe(false);
    });

    it('not in: string not in list', () => {
        expect(eval_("y not in ('one','two','tree')", {
            y: atomString('four'),
        })).toBe(true);
        expect(eval_("y not in ('one','two','tree')", {
            y: atomString('one'),
        })).toBe(false);
    });

    it('variable as string matching unquoted list items', () => {
        expect(eval_('y in (one,two,tree)', {
            y: atomString('two'),
        })).toBe(true);
    });

    it('missing variable with in returns false', () => {
        expect(eval_("z in ('a','b')", {})).toBe(false);
    });

    it('missing variable with not in returns false', () => {
        expect(eval_("z not in ('a','b')", {})).toBe(false);
    });
});

// ── Match (contains / regex) ──────────────────────────────────────

describe('match contains and regex', () => {
    it('contains: true when substring found', () => {
        expect(eval_('name ~ Nik', { name: atomString('Nikolajus') })).toBe(true);
    });

    it('contains: false when substring not found', () => {
        expect(eval_('name ~ Nik', { name: atomString('John') })).toBe(false);
    });

    it('not contains: true when substring not found', () => {
        expect(eval_('name !~ Nik', { name: atomString('John') })).toBe(true);
    });

    it('not contains: false when substring found', () => {
        expect(eval_('name !~ Nik', { name: atomString('Nikolajus') })).toBe(false);
    });

    it('regex match: true when pattern matches', () => {
        expect(eval_('name ~ /.*ola.*/', { name: atomString('Nikolajus') })).toBe(true);
    });

    it('regex match: false when pattern does not match', () => {
        expect(eval_('name ~ /.*ola.*/', { name: atomString('John') })).toBe(false);
    });

    it('not regex: true when pattern does not match', () => {
        expect(eval_('name !~ /.*ola.*/', { name: atomString('John') })).toBe(true);
    });

    it('not regex: false when pattern matches', () => {
        expect(eval_('name !~ /.*ola.*/', { name: atomString('Nikolajus') })).toBe(false);
    });

    it('missing variable returns false', () => {
        expect(eval_('name ~ Nik', {})).toBe(false);
    });
});

// ── StartsWith / EndsWith ─────────────────────────────────────────

describe('startsWith and endsWith', () => {
    it('startsWith: true when string starts with prefix', () => {
        expect(eval_('path ^~ "/admin"', { path: atomString('/admin/settings') })).toBe(true);
    });

    it('startsWith: false when string does not start with prefix', () => {
        expect(eval_('path ^~ "/admin"', { path: atomString('/user/profile') })).toBe(false);
    });

    it('endsWith: true when string ends with suffix', () => {
        expect(eval_('email ~$ "@company.com"', { email: atomString('user@company.com') })).toBe(true);
    });

    it('endsWith: false when string does not end with suffix', () => {
        expect(eval_('email ~$ "@company.com"', { email: atomString('user@other.com') })).toBe(false);
    });

    it('notStartsWith: true when string does not start with prefix', () => {
        expect(eval_('name !^~ "test"', { name: atomString('production') })).toBe(true);
    });

    it('notStartsWith: false when string starts with prefix', () => {
        expect(eval_('name !^~ "test"', { name: atomString('testing123') })).toBe(false);
    });

    it('notEndsWith: true when string does not end with suffix', () => {
        expect(eval_('name !~$ ".tmp"', { name: atomString('file.txt') })).toBe(true);
    });

    it('notEndsWith: false when string ends with suffix', () => {
        expect(eval_('name !~$ ".tmp"', { name: atomString('data.tmp') })).toBe(false);
    });

    it('edge: empty string startsWith/endsWith empty string → true', () => {
        expect(eval_('name ^~ ""', { name: atomString('') })).toBe(true);
        expect(eval_('name ~$ ""', { name: atomString('') })).toBe(true);
    });

    it('edge: exact match (string equals prefix/suffix entirely) → true', () => {
        expect(eval_('name ^~ "hello"', { name: atomString('hello') })).toBe(true);
        expect(eval_('name ~$ "hello"', { name: atomString('hello') })).toBe(true);
    });

    it('combined with logic: path ^~ "/api" and method == "GET"', () => {
        expect(eval_('path ^~ "/api" and method == "GET"', {
            path: atomString('/api/users'),
            method: atomString('GET'),
        })).toBe(true);
        expect(eval_('path ^~ "/api" and method == "GET"', {
            path: atomString('/home'),
            method: atomString('GET'),
        })).toBe(false);
    });

    it('combined with function: lower(name) ^~ "admin"', () => {
        expect(eval_('lower(name) ^~ "admin"', { name: atomString('ADMIN_USER') })).toBe(true);
        expect(eval_('lower(name) ^~ "admin"', { name: atomString('USER_ADMIN') })).toBe(false);
    });

    it('missing variable returns false', () => {
        expect(eval_('name ^~ "test"', {})).toBe(false);
        expect(eval_('name ~$ "test"', {})).toBe(false);
    });
});

// ── Semver comparisons ────────────────────────────────────────────

describe('semver comparisons', () => {
    it('semver > semver', () => {
        expect(eval_('version > 5.3.42', {
            version: atomSemver(6, 0, 0),
        })).toBe(true);
        expect(eval_('version > 5.3.42', {
            version: atomSemver(5, 3, 42),
        })).toBe(false);
        expect(eval_('version > 5.3.42', {
            version: atomSemver(5, 3, 43),
        })).toBe(true);
    });

    it('semver < semver', () => {
        expect(eval_('version < 4.32.0', {
            version: atomSemver(4, 31, 9),
        })).toBe(true);
        expect(eval_('version < 4.32.0', {
            version: atomSemver(4, 32, 0),
        })).toBe(false);
    });

    it('semver == semver', () => {
        expect(eval_('version == 1.2.3', {
            version: atomSemver(1, 2, 3),
        })).toBe(true);
        expect(eval_('version == 1.2.3', {
            version: atomSemver(1, 2, 4),
        })).toBe(false);
    });

    it('semver >= and <=', () => {
        expect(eval_('version >= 2.0.0', {
            version: atomSemver(2, 0, 0),
        })).toBe(true);
        expect(eval_('version <= 2.0.0', {
            version: atomSemver(1, 9, 99),
        })).toBe(true);
    });

    it('float coerced to semver: 5.4 → (5,4,0)', () => {
        expect(eval_('version > 5.3.42', {
            version: atomFloat(5.4),
        })).toBe(true);
        expect(eval_('version > 5.3.42', {
            version: atomFloat(5.3),
        })).toBe(false);
        expect(eval_('version == 5.4.0', {
            version: atomFloat(5.4),
        })).toBe(true);
    });
});

// ── Date comparisons ──────────────────────────────────────────────

describe('date comparisons', () => {
    it('date range comparison', () => {
        expect(eval_('created > 2024-02-02 and created <= 2024-02-13', {
            created: atomDate('2024-02-12'),
        })).toBe(true);
    });

    it('date not less than itself', () => {
        expect(eval_('created < 2024-02-02', {
            created: atomDate('2024-02-02'),
        })).toBe(false);
    });
});

// ── Variable resolution with boolean context ──────────────────────

describe('variable resolution', () => {
    it('boolean variable resolves from context', () => {
        expect(eval_('enabled', {
            enabled: atomBoolean(true),
        })).toBe(true);
        expect(eval_('enabled', {
            enabled: atomBoolean(false),
        })).toBe(false);
    });

    it('missing boolean variable returns false', () => {
        expect(eval_('missing', {})).toBe(false);
    });
});

// ── Percentage rollout ────────────────────────────────────────────

describe('percentage rollout', () => {
    it('percentage 0% is always false', () => {
        const r = parse('percentage(0%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        const result = evaluate(r.value, { userId: atomString('user-123') }, 'FF-test-rollout');
        expect(result).toBe(false);
    });

    it('percentage 100% is always true', () => {
        const r = parse('percentage(100%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        const result = evaluate(r.value, { userId: atomString('user-123') }, 'FF-test-rollout');
        expect(result).toBe(true);
    });

    it('percentage is deterministic', () => {
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        const ctx = { userId: atomString('alice') };
        const r1 = evaluate(r.value, ctx, 'FF-test');
        const r2 = evaluate(r.value, ctx, 'FF-test');
        expect(r1).toBe(r2);
    });

    it('percentage with missing variable returns false', () => {
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, {}, 'FF-test')).toBe(false);
    });

    it('percentage with salt produces different bucketing', () => {
        const r1 = parse('percentage(50%, userId)');
        const r2 = parse('percentage(50%, userId, experiment_a)');
        expect(r1.ok).toBe(true);
        expect(r2.ok).toBe(true);
        if (!r1.ok || !r2.ok) return;
        // With enough users, salted vs unsalted should differ for at least some
        let differ = false;
        for (let i = 0; i < 100; i++) {
            const ctx = { userId: atomString(`user-${i}`) };
            const a = evaluate(r1.value, ctx, 'FF-test');
            const b = evaluate(r2.value, ctx, 'FF-test');
            if (a !== b) { differ = true; break; }
        }
        expect(differ).toBe(true);
    });

    // Cross-language test vectors (MUST match Rust implementation exactly)
    // SHA-1 hex → first 15 chars → parse base-16 → mod 100000 = bucket → bucket < rate*1000

    it('cross-language vector 1: FF-test-rollout.user-123 at 50% → true', () => {
        // SHA-1("FF-test-rollout.user-123") = 60feafb1513ee86...
        // bucket = 436826052989546118 % 100000 = 46118 < 50000 → true
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, { userId: atomString('user-123') }, 'FF-test-rollout')).toBe(true);
    });

    it('cross-language vector 2: FF-test-rollout.user-456 at 50% → false', () => {
        // SHA-1("FF-test-rollout.user-456") = 66438f4ed936777...
        // bucket = 460555686507669367 % 100000 = 69367 >= 50000 → false
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, { userId: atomString('user-456') }, 'FF-test-rollout')).toBe(false);
    });

    it('cross-language vector 3: FF-new-checkout.user-789 at 50% → true', () => {
        // SHA-1("FF-new-checkout.user-789") = 57fc354f1e45f99...
        // bucket = 396250061834837913 % 100000 = 37913 < 50000 → true
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, { userId: atomString('user-789') }, 'FF-new-checkout')).toBe(true);
    });

    it('cross-language vector 4: with salt exp1, FF-test-rollout.exp1.alice at 50% → false', () => {
        // SHA-1("FF-test-rollout.exp1.alice") = 8f91f05372579e5...
        // bucket = 646582128764877285 % 100000 = 77285 >= 50000 → false
        const r = parse('percentage(50%, userId, exp1)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, { userId: atomString('alice') }, 'FF-test-rollout')).toBe(false);
    });

    it('cross-language vector 5: FF-test.alice at 50% → false', () => {
        // SHA-1("FF-test.alice") = 76706ecbaa75e55...
        // bucket = 533402694680272469 % 100000 = 72469 >= 50000 → false
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        expect(evaluate(r.value, { userId: atomString('alice') }, 'FF-test')).toBe(false);
    });

    it('percentage distribution is roughly correct', () => {
        const r = parse('percentage(50%, userId)');
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        let trueCount = 0;
        const total = 10000;
        for (let i = 0; i < total; i++) {
            const ctx = { userId: atomString(`user-${i}`) };
            if (evaluate(r.value, ctx, 'FF-distribution-test')) trueCount++;
        }
        // Should be roughly 50% (within 5% tolerance)
        expect(trueCount / total).toBeGreaterThan(0.45);
        expect(trueCount / total).toBeLessThan(0.55);
    });
});
