import { describe, it, expect, beforeEach } from 'vitest';
import { init, initFromString, initFromStringWithEnv, ff, ffRaw, ffMetadata, _reset } from './index.js';

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

describe('ffMetadata() — flag metadata', () => {
    it('returns metadata for annotated flag', () => {
        initFromString(`@owner "payments-team"
@expires 2026-06-01
@ticket "JIRA-1234"
@description "New 3DS2 auth flow"
@type release
FF-3ds2-auth -> true`);
        const meta = ffMetadata('FF-3ds2-auth');
        expect(meta).not.toBeNull();
        expect(meta!.owner).toBe('payments-team');
        expect(meta!.expires).toBe('2026-06-01');
        expect(meta!.ticket).toBe('JIRA-1234');
        expect(meta!.description).toBe('New 3DS2 auth flow');
        expect(meta!.flagType).toBe('release');
    });

    it('returns empty metadata for unannotated flag', () => {
        initFromString('FF-simple -> true');
        const meta = ffMetadata('FF-simple');
        expect(meta).toEqual({});
    });

    it('returns null for unknown flag', () => {
        initFromString('FF-x -> true');
        expect(ffMetadata('FF-nonexistent')).toBeNull();
    });

    it('deprecated flag still evaluates normally', () => {
        initFromString(`@deprecated "Use FF-new instead"
FF-old -> true`);
        expect(ff('FF-old')).toBe(true);
        expect(ffMetadata('FF-old')!.deprecated).toBe('Use FF-new instead');
    });
});

describe('@requires — flag prerequisites', () => {
    it('evaluates flag when prerequisite is true', () => {
        initFromString(`FF-new-checkout -> true

@requires FF-new-checkout
FF-checkout-upsell -> true`);
        expect(ff('FF-checkout-upsell')).toBe(true);
    });

    it('returns null when prerequisite is false', () => {
        initFromString(`FF-new-checkout -> false

@requires FF-new-checkout
FF-checkout-upsell -> true`);
        expect(ff('FF-checkout-upsell')).toBeNull();
    });

    it('returns null when prerequisite flag is missing', () => {
        initFromString(`@requires FF-nonexistent
FF-dependent -> true`);
        expect(ff('FF-dependent')).toBeNull();
    });

    it('evaluates when all prerequisites are true', () => {
        initFromString(`FF-base -> true
FF-premium -> true

@requires FF-base
@requires FF-premium
FF-advanced -> true`);
        expect(ff('FF-advanced')).toBe(true);
    });

    it('returns null when any prerequisite is false', () => {
        initFromString(`FF-base -> true
FF-premium -> false

@requires FF-base
@requires FF-premium
FF-advanced -> true`);
        expect(ff('FF-advanced')).toBeNull();
    });

    it('prerequisite evaluated with same context', () => {
        initFromString(`FF-gate {
    plan == premium -> true
    false
}

@requires FF-gate
FF-premium-feature -> true`);
        expect(ff('FF-premium-feature', { plan: 'premium' })).toBe(true);
        expect(ff('FF-premium-feature', { plan: 'free' })).toBeNull();
    });

    it('ffRaw also checks prerequisites', () => {
        initFromString(`FF-gate -> false

@requires FF-gate
FF-gated -> true`);
        expect(ffRaw('FF-gated')).toBeNull();
    });
});

describe('@env — environment-based rules', () => {
    it('matches simple env rule', () => {
        initFromStringWithEnv(`FF-debug {
    @env dev -> true
    @env prod -> false
}`, 'dev');
        expect(ff('FF-debug')).toBe(true);
    });

    it('matches different env', () => {
        initFromStringWithEnv(`FF-debug {
    @env dev -> true
    @env prod -> false
}`, 'prod');
        expect(ff('FF-debug')).toBe(false);
    });

    it('skips env rules when no env is set', () => {
        initFromString(`FF-debug {
    @env dev -> true
    false
}`);
        expect(ff('FF-debug')).toBe(false);
    });

    it('falls through to default when env does not match', () => {
        initFromStringWithEnv(`FF-debug {
    @env dev -> true
    false
}`, 'staging');
        expect(ff('FF-debug')).toBe(false);
    });

    it('handles env block form with sub-rules', () => {
        initFromStringWithEnv(`FF-feature {
    @env prod {
        plan == premium -> true
        false
    }
    true
}`, 'prod');
        expect(ff('FF-feature', { plan: 'premium' })).toBe(true);
        expect(ff('FF-feature', { plan: 'free' })).toBe(false);
    });

    it('skips env block when env does not match', () => {
        initFromStringWithEnv(`FF-feature {
    @env prod {
        plan == premium -> true
        false
    }
    true
}`, 'dev');
        expect(ff('FF-feature')).toBe(true);
    });

    it('multiple env rules with fallback', () => {
        initFromStringWithEnv(`FF-logging {
    @env dev -> true
    @env stage -> true
    @env prod -> false
}`, 'stage');
        expect(ff('FF-logging')).toBe(true);
    });

    it('env rules work with ffRaw', () => {
        initFromStringWithEnv(`FF-level {
    @env dev -> "debug"
    "info"
}`, 'dev');
        expect(ffRaw('FF-level')).toEqual({ type: 'Str', value: 'debug' });
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
