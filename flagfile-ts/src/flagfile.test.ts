import { describe, it, expect } from 'vitest';
import { parseFlagfile } from './flagfile.js';
import { readFileSync } from 'fs';
import { join } from 'path';

describe('parseFlagfile', () => {
    it('parses short notation', () => {
        const r = parseFlagfile('FF-new-ui -> true');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            const rules = r.value.get('FF-new-ui');
            expect(rules).toBeDefined();
            expect(rules!.length).toBe(1);
            expect(rules![0]).toEqual({
                type: 'Value',
                value: { type: 'OnOff', value: true },
            });
        }
    });

    it('parses block notation', () => {
        const data = `FF-feature-y {
    countryCode == NL -> true
    false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            const rules = r.value.get('FF-feature-y');
            expect(rules).toBeDefined();
            expect(rules!.length).toBe(2);
            expect(rules![0].type).toBe('BoolExpressionValue');
            expect(rules![1].type).toBe('Value');
        }
    });

    it('parses snake_case flag names', () => {
        const data = `FF_feature_y {
    FALSE
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            expect(r.value.has('FF_feature_y')).toBe(true);
        }
    });

    it('parses integer return', () => {
        const data = 'FF-api-timeout -> 5000';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const rules = r.value.get('FF-api-timeout')!;
            expect(rules.length).toBe(1);
            expect(rules[0]).toEqual({
                type: 'Value',
                value: { type: 'Integer', value: 5000 },
            });
        }
    });

    it('parses string return', () => {
        const data = 'FF-log-level -> "debug"';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const rules = r.value.get('FF-log-level')!;
            expect(rules.length).toBe(1);
            expect(rules[0]).toEqual({
                type: 'Value',
                value: { type: 'Str', value: 'debug' },
            });
        }
    });

    it('parses integer in block', () => {
        const data = `FF-timeout {
    plan == premium -> 10000
    5000
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const rules = r.value.get('FF-timeout')!;
            expect(rules.length).toBe(2);
            expect(rules[1]).toEqual({
                type: 'Value',
                value: { type: 'Integer', value: 5000 },
            });
        }
    });

    it('parses json return', () => {
        const data = 'FF-feature-json-variant -> json({"success": true})';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const rules = r.value.get('FF-feature-json-variant')!;
            expect(rules.length).toBe(1);
            expect(rules[0]).toEqual({
                type: 'Value',
                value: { type: 'Json', value: { success: true } },
            });
        }
    });

    it('handles comments', () => {
        const data = `// this is a comment
FF-new-ui -> true
/* block comment */
FF-beta -> false`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(2);
        }
    });

    it('parses full Flagfile.example', () => {
        const data = readFileSync(
            join(__dirname, '../../Flagfile.example'),
            'utf-8',
        );
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBeGreaterThan(0);
            expect(r.rest.trim()).toBe('');
        }
    });
});
