import { describe, it, expect } from 'vitest';
import { parse, parseAtom } from './parser.js';
import { ComparisonOp, LogicOp, ArrayOp, MatchOp, FnCall } from './ast.js';

describe('parseAtom', () => {
    it('parses booleans', () => {
        const r = parseAtom('true');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value).toEqual({ type: 'Boolean', value: true });
            expect(r.rest).toBe('');
        }

        const r2 = parseAtom('FALSE');
        expect(r2.ok).toBe(true);
        if (r2.ok) expect(r2.value).toEqual({ type: 'Boolean', value: false });
    });

    it('parses numbers', () => {
        const r = parseAtom('-10');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Number', value: -10 });

        const r2 = parseAtom('199');
        expect(r2.ok).toBe(true);
        if (r2.ok) expect(r2.value).toEqual({ type: 'Number', value: 199 });
    });

    it('parses floats', () => {
        const r = parseAtom('3.14');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Float', value: 3.14 });
    });

    it('parses integer 3 as Number not Float', () => {
        const r = parseAtom('3');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Number', value: 3 });
    });

    it('parses strings', () => {
        const r = parseAtom('"this is demo"');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'String', value: 'this is demo' });
    });

    it('parses single-quoted strings', () => {
        const r = parseAtom("'hello world'");
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'String', value: 'hello world' });
    });

    it('parses variables', () => {
        const r = parseAtom('_demo_demo');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Variable', value: '_demo_demo' });
    });

    it('parses dates', () => {
        const r = parseAtom('2004-12-23');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Date', value: '2004-12-23' });
    });

    it('parses semver', () => {
        const r = parseAtom('5.3.42');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.value).toEqual({ type: 'Semver', major: 5, minor: 3, patch: 42 });

        // 2-component parses as float
        const r2 = parseAtom('4.32');
        expect(r2.ok).toBe(true);
        if (r2.ok) expect(r2.value).toEqual({ type: 'Float', value: 4.32 });
    });
});

describe('parse expressions', () => {
    it('parses constant true', () => {
        const r = parse('True');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.type).toBe('Constant');
        }
    });

    it('parses comparison', () => {
        const r = parse('_demo >= 10');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Compare');
        }
    });

    it('parses logic expression', () => {
        const r = parse('_demo >= 10 && demo == "something more than that"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Logic');
        }
    });

    it('parses list', () => {
        const r = parse('a in (1,2, 34, "demo", -10, -3.14)');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses logic with list', () => {
        const r = parse('a = 2 and b in (1,2.2, "demo")');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses complex not in', () => {
        const r = parse('a=3 && c = 3 || d not in (2,4,5) and this<>34.43');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses scope with not', () => {
        const r = parse('not (a=b and c=d)');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.type).toBe('Scope');
            if (r.value.type === 'Scope') {
                expect(r.value.negate).toBe(true);
            }
        }
    });

    it('parses function modifiers', () => {
        const r = parse("UPPER(_demo) == 'DEMO DEMO'");
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses single quote in comparison', () => {
        const r = parse("a='demo demo'");
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses semver comparison', () => {
        const r = parse('version > 5.3.42');
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');

        const r2 = parse('appVersion <= 4.32.0');
        expect(r2.ok).toBe(true);
        if (r2.ok) expect(r2.rest).toBe('');
    });

    it('parses extreme logic test', () => {
        const expression = `a = b and c=d and something not in (1,2,3) or lower(z) == "demo car" or
    z == "demo car" or
    g in (4,5,6) and z == "demo car" or
    model in (ms,mx,m3,my) and !(created >= 2024-01-01
        and demo == false) and ((a=2) and not (c=3))`;
        const r = parse(expression);
        expect(r.ok).toBe(true);
        if (r.ok) expect(r.rest).toBe('');
    });

    it('parses NOW() function', () => {
        const r = parse('NOW() > 2026-02-07');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Compare');
        }
    });

    it('parses match contains: name ~ Nik', () => {
        const r = parse('name ~ Nik');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.Contains);
            }
        }
    });

    it('parses match not contains: name !~ Nik', () => {
        const r = parse('name !~ Nik');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.NotContains);
            }
        }
    });

    it('parses match regex: name ~ /.*ola.*/', () => {
        const r = parse('name ~ /.*ola.*/');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.Contains);
                expect(r.value.right.type).toBe('Constant');
                if (r.value.right.type === 'Constant') {
                    expect(r.value.right.atom.type).toBe('Regex');
                }
            }
        }
    });

    it('parses match not regex: name !~ /.*ola.*/', () => {
        const r = parse('name !~ /.*ola.*/');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.NotContains);
            }
        }
    });

    it('parses match in logic expr: name ~ Nik and age > 18', () => {
        const r = parse('name ~ Nik and age > 18');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Logic');
        }
    });

    it('parses startsWith: path ^~ "/admin"', () => {
        const r = parse('path ^~ "/admin"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.StartsWith);
            }
        }
    });

    it('parses endsWith: email ~$ "@company.com"', () => {
        const r = parse('email ~$ "@company.com"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.EndsWith);
            }
        }
    });

    it('parses notStartsWith: name !^~ "test"', () => {
        const r = parse('name !^~ "test"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.NotStartsWith);
            }
        }
    });

    it('parses notEndsWith: name !~$ ".tmp"', () => {
        const r = parse('name !~$ ".tmp"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.NotEndsWith);
            }
        }
    });

    it('parses startsWith in logic expr: path ^~ "/api" and method == "GET"', () => {
        const r = parse('path ^~ "/api" and method == "GET"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Logic');
        }
    });

    it('parses startsWith with function: lower(name) ^~ "admin"', () => {
        const r = parse('lower(name) ^~ "admin"');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Match');
            if (r.value.type === 'Match') {
                expect(r.value.op).toBe(MatchOp.StartsWith);
            }
        }
    });

    it('rejects array with comparison op', () => {
        // "a == 2 and b >= (1,2,3)" should not fully parse
        const r = parse('a == 2 and b >= (1,2,3)');
        // The parser should either fail or leave a non-empty rest
        if (r.ok) {
            expect(r.rest.trim()).not.toBe('');
        }
    });

    it('parses percentage(5%, userId)', () => {
        const r = parse('percentage(5%, userId)');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Percentage');
            if (r.value.type === 'Percentage') {
                expect(r.value.rate).toBe(5);
                expect(r.value.salt).toBeNull();
            }
        }
    });

    it('parses percentage(50%, orgId, custom_salt)', () => {
        const r = parse('percentage(50%, orgId, custom_salt)');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Percentage');
            if (r.value.type === 'Percentage') {
                expect(r.value.rate).toBe(50);
                expect(r.value.salt).toBe('custom_salt');
            }
        }
    });

    it('parses percentage(0.5%, userId) with decimal rate', () => {
        const r = parse('percentage(0.5%, userId)');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Percentage');
            if (r.value.type === 'Percentage') {
                expect(r.value.rate).toBe(0.5);
            }
        }
    });

    it('parses percentage combined with logic', () => {
        const r = parse('percentage(50%, orgId) and plan == premium');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Logic');
        }
    });

    it('parses percentage(100%, userId)', () => {
        const r = parse('percentage(100%, userId)');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.rest).toBe('');
            expect(r.value.type).toBe('Percentage');
            if (r.value.type === 'Percentage') {
                expect(r.value.rate).toBe(100);
            }
        }
    });
});
