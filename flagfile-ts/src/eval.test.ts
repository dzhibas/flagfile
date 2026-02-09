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
