import { describe, it, expect, beforeEach } from 'vitest';
import { init, initFromString, ff, ffRaw, _reset } from './index.js';

beforeEach(() => {
    _reset();
});

describe('ff() — plain JS values', () => {
    it('returns true for a simple on flag', () => {
        initFromString('FF-hello -> true');
        expect(ff('FF-hello')).toBe(true);
    });

    it('returns false for a simple off flag', () => {
        initFromString('FF-off -> false');
        expect(ff('FF-off')).toBe(false);
    });

    it('evaluates flag with plain string context', () => {
        initFromString(`FF-premium {
    plan == premium -> true
    false
}`);
        expect(ff('FF-premium', { plan: 'premium' })).toBe(true);
        expect(ff('FF-premium', { plan: 'free' })).toBe(false);
    });

    it('returns null for unknown flag', () => {
        initFromString('FF-hello -> true');
        expect(ff('FF-nonexistent')).toBeNull();
    });

    it('returns number for integer flag', () => {
        initFromString('FF-timeout -> 5000');
        expect(ff('FF-timeout')).toBe(5000);
    });

    it('returns string for string flag', () => {
        initFromString('FF-level -> "debug"');
        expect(ff('FF-level')).toBe('debug');
    });

    it('context is optional', () => {
        initFromString('FF-on -> true');
        expect(ff('FF-on')).toBe(true);
    });

    it('works in a plain if-statement', () => {
        initFromString('FF-feature -> true');
        if (ff('FF-feature')) {
            expect(true).toBe(true);
        } else {
            expect.unreachable('should not reach here');
        }
    });

    it('handles in-list membership with plain strings', () => {
        initFromString(`FF-region {
    country in (NL, BE, DE) -> true
    false
}`);
        expect(ff('FF-region', { country: 'NL' })).toBe(true);
        expect(ff('FF-region', { country: 'US' })).toBe(false);
    });

    it('handles numeric context values', () => {
        initFromString(`FF-big-spender {
    amount > 100 -> true
    false
}`);
        expect(ff('FF-big-spender', { amount: 200 })).toBe(true);
        expect(ff('FF-big-spender', { amount: 50 })).toBe(false);
    });

    it('handles boolean context values', () => {
        initFromString(`FF-admin {
    isAdmin == true -> true
    false
}`);
        expect(ff('FF-admin', { isAdmin: true })).toBe(true);
        expect(ff('FF-admin', { isAdmin: false })).toBe(false);
    });
});

describe('ffRaw() — FlagReturn objects', () => {
    it('returns FlagReturn for OnOff', () => {
        initFromString('FF-on -> true');
        expect(ffRaw('FF-on')).toEqual({ type: 'OnOff', value: true });
    });

    it('returns FlagReturn for Integer', () => {
        initFromString('FF-timeout -> 5000');
        expect(ffRaw('FF-timeout')).toEqual({ type: 'Integer', value: 5000 });
    });

    it('returns FlagReturn for Str', () => {
        initFromString('FF-level -> "debug"');
        expect(ffRaw('FF-level')).toEqual({ type: 'Str', value: 'debug' });
    });

    it('returns null for unknown flag', () => {
        initFromString('FF-x -> true');
        expect(ffRaw('FF-nope')).toBeNull();
    });
});

describe('init lifecycle', () => {
    it('throws if called twice', () => {
        initFromString('FF-a -> true');
        expect(() => initFromString('FF-b -> true')).toThrow(
            'init() or initFromString() was called more than once',
        );
    });

    it('throws if ff called before init', () => {
        expect(() => ff('FF-a')).toThrow(
            'init() or initFromString() must be called before ff()',
        );
    });
});
