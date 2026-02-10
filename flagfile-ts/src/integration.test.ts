import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join } from 'path';
import { parseFlagfile } from './flagfile.js';
import { evaluate, Context } from './eval.js';
import { parse } from './parser.js';
import {
    Atom,
    FlagReturn,
    Rule,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
    atomSemver,
    atomDate,
} from './ast.js';

function evaluateFlag(
    rules: Rule[],
    context: Context,
): FlagReturn | null {
    for (const rule of rules) {
        if (rule.type === 'Value') {
            return rule.value;
        }
        if (rule.type === 'BoolExpressionValue') {
            if (evaluate(rule.expr, context)) {
                return rule.value;
            }
        }
    }
    return null;
}

describe('integration: Flagfile.example parsing', () => {
    const data = readFileSync(
        join(__dirname, '../../Flagfile.example'),
        'utf-8',
    );
    const result = parseFlagfile(data);

    it('parses Flagfile.example completely', () => {
        expect(result.ok).toBe(true);
        if (result.ok) {
            expect(result.rest.trim()).toBe('');
            expect(result.value.size).toBeGreaterThan(10);
        }
    });

    it('has expected flags', () => {
        if (!result.ok) return;
        const flags = result.value;
        expect(flags.has('FF-new-ui')).toBe(true);
        expect(flags.has('FF-beta-features')).toBe(true);
        expect(flags.has('FF-maintenance-mode')).toBe(true);
        expect(flags.has('FF-api-timeout')).toBe(true);
        expect(flags.has('FF-max-retries')).toBe(true);
        expect(flags.has('FF-log-level')).toBe(true);
        expect(flags.has('FF-feature-y')).toBe(true);
        expect(flags.has('FF-feature-complex-ticket-234234')).toBe(true);
        expect(flags.has('FF-sdk-upgrade')).toBe(true);
        expect(flags.has('FF-timer-feature')).toBe(true);
        expect(flags.has('FF-contains-feature-check')).toBe(true);
        expect(flags.has('FF-regexp-feature-check')).toBe(true);
    });
});

describe('integration: Flagfile.tests validation', () => {
    const data = readFileSync(
        join(__dirname, '../../Flagfile.example'),
        'utf-8',
    );
    const result = parseFlagfile(data);
    if (!result.ok) throw new Error('Failed to parse Flagfile.example');
    const flags = result.value;

    // FF-feature-complex-ticket-234234(model=my,created=2024-03-03,demo=false) == true
    it('FF-feature-complex-ticket-234234 with model=my,created=2024-03-03,demo=false → true', () => {
        const rules = flags.get('FF-feature-complex-ticket-234234')!;
        const ctx: Context = {
            model: atomString('my'),
            created: atomDate('2024-03-03'),
            demo: atomBoolean(false),
        };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-feature-complex-ticket-234234(model=my,created=2023-06-15,demo=false) == false
    it('FF-feature-complex-ticket-234234 with model=my,created=2023-06-15,demo=false → false', () => {
        const rules = flags.get('FF-feature-complex-ticket-234234')!;
        const ctx: Context = {
            model: atomString('my'),
            created: atomDate('2023-06-15'),
            demo: atomBoolean(false),
        };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-sdk-upgrade(appVersion=6.0.0) == TRUE
    it('FF-sdk-upgrade with appVersion=6.0.0 → true', () => {
        const rules = flags.get('FF-sdk-upgrade')!;
        const ctx: Context = { appVersion: atomSemver(6, 0, 0) };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-sdk-upgrade(appVersion=4.31.9) == FALSE
    it('FF-sdk-upgrade with appVersion=4.31.9 → false', () => {
        const rules = flags.get('FF-sdk-upgrade')!;
        const ctx: Context = { appVersion: atomSemver(4, 31, 9) };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-sdk-upgrade(appVersion=5.3.44) == TRUE
    it('FF-sdk-upgrade with appVersion=5.3.44 → true', () => {
        const rules = flags.get('FF-sdk-upgrade')!;
        const ctx: Context = { appVersion: atomSemver(5, 3, 44) };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-sdk-upgrade(appVersion=5.2.1) == false
    it('FF-sdk-upgrade with appVersion=5.2.1 → false', () => {
        const rules = flags.get('FF-sdk-upgrade')!;
        const ctx: Context = { appVersion: atomSemver(5, 2, 1) };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-feature-y(countryCode=nl) == true
    it('FF-feature-y with countryCode=nl → true', () => {
        const rules = flags.get('FF-feature-y')!;
        const ctx: Context = { countryCode: atomString('nl') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-feature-y(countryCode=NL) == true
    it('FF-feature-y with countryCode=NL → true', () => {
        const rules = flags.get('FF-feature-y')!;
        const ctx: Context = { countryCode: atomString('NL') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-feature-y(countryCode=DE) == false
    it('FF-feature-y with countryCode=DE → false', () => {
        const rules = flags.get('FF-feature-y')!;
        const ctx: Context = { countryCode: atomString('DE') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-api-timeout == 5000
    it('FF-api-timeout → 5000', () => {
        const rules = flags.get('FF-api-timeout')!;
        const val = evaluateFlag(rules, {});
        expect(val).toEqual({ type: 'Integer', value: 5000 });
    });

    // FF-max-retries == 3
    it('FF-max-retries → 3', () => {
        const rules = flags.get('FF-max-retries')!;
        const val = evaluateFlag(rules, {});
        expect(val).toEqual({ type: 'Integer', value: 3 });
    });

    // FF-log-level == "debug"
    it('FF-log-level → "debug"', () => {
        const rules = flags.get('FF-log-level')!;
        const val = evaluateFlag(rules, {});
        expect(val).toEqual({ type: 'Str', value: 'debug' });
    });

    // FF-contains-feature-check(name="Nikolajus") == true
    it('FF-contains-feature-check with name="Nikolajus" → true', () => {
        const rules = flags.get('FF-contains-feature-check')!;
        const ctx: Context = { name: atomString('Nikolajus') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-regexp-feature-check(name="Check Nikolajus match") == true
    it('FF-regexp-feature-check with name="Check Nikolajus match" → true', () => {
        const rules = flags.get('FF-regexp-feature-check')!;
        const ctx: Context = { name: atomString('Check Nikolajus match') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-email-domain-check(email="user@company.com") == true
    it('FF-email-domain-check with email="user@company.com" → true', () => {
        const rules = flags.get('FF-email-domain-check')!;
        const ctx: Context = { email: atomString('user@company.com') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-email-domain-check(email="user@other.com") == false
    it('FF-email-domain-check with email="user@other.com" → false', () => {
        const rules = flags.get('FF-email-domain-check')!;
        const ctx: Context = { email: atomString('user@other.com') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-admin-path-check(path="/admin/settings") == true
    it('FF-admin-path-check with path="/admin/settings" → true', () => {
        const rules = flags.get('FF-admin-path-check')!;
        const ctx: Context = { path: atomString('/admin/settings') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: true });
    });

    // FF-admin-path-check(path="/user/profile") == false
    it('FF-admin-path-check with path="/user/profile" → false', () => {
        const rules = flags.get('FF-admin-path-check')!;
        const ctx: Context = { path: atomString('/user/profile') };
        const val = evaluateFlag(rules, ctx);
        expect(val).toEqual({ type: 'OnOff', value: false });
    });

    // FF-button-color == "blue"
    it('FF-button-color → "blue"', () => {
        const rules = flags.get('FF-button-color')!;
        const val = evaluateFlag(rules, {});
        expect(val).toEqual({ type: 'Str', value: 'blue' });
    });

    // FF-theme-config() == json
    it('FF-theme-config → json with correct structure', () => {
        const rules = flags.get('FF-theme-config')!;
        const val = evaluateFlag(rules, {});
        expect(val).not.toBeNull();
        expect(val!.type).toBe('Json');
        if (val!.type === 'Json') {
            expect(val!.value).toEqual({
                primaryColor: '#007bff',
                secondaryColor: '#6c757d',
                darkMode: true,
                animations: true,
            });
        }
    });
});
