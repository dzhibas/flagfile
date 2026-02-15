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
            const def = r.value.get('FF-new-ui');
            expect(def).toBeDefined();
            expect(def!.rules.length).toBe(1);
            expect(def!.rules[0]).toEqual({
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
            const def = r.value.get('FF-feature-y');
            expect(def).toBeDefined();
            expect(def!.rules.length).toBe(2);
            expect(def!.rules[0].type).toBe('BoolExpressionValue');
            expect(def!.rules[1].type).toBe('Value');
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
            const def = r.value.get('FF-api-timeout')!;
            expect(def.rules.length).toBe(1);
            expect(def.rules[0]).toEqual({
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
            const def = r.value.get('FF-log-level')!;
            expect(def.rules.length).toBe(1);
            expect(def.rules[0]).toEqual({
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
            const def = r.value.get('FF-timeout')!;
            expect(def.rules.length).toBe(2);
            expect(def.rules[1]).toEqual({
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
            const def = r.value.get('FF-feature-json-variant')!;
            expect(def.rules.length).toBe(1);
            expect(def.rules[0]).toEqual({
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

    // ── Metadata annotation tests ─────────────────────────────────

    it('parses @owner annotation', () => {
        const data = '@owner "payments-team"\nFF-pay -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-pay')!;
            expect(def.metadata.owner).toBe('payments-team');
            expect(def.rules.length).toBe(1);
        }
    });

    it('parses @expires annotation', () => {
        const data = '@expires 2026-06-01\nFF-temp -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-temp')!;
            expect(def.metadata.expires).toBe('2026-06-01');
        }
    });

    it('parses multiple annotations on one flag', () => {
        const data = `@owner "payments-team"
@expires 2026-06-01
@ticket "JIRA-1234"
@description "New 3DS2 auth flow"
@type release
FF-3ds2-auth {
    percentage(50%, userId) -> true
    false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-3ds2-auth')!;
            expect(def.metadata.owner).toBe('payments-team');
            expect(def.metadata.expires).toBe('2026-06-01');
            expect(def.metadata.ticket).toBe('JIRA-1234');
            expect(def.metadata.description).toBe('New 3DS2 auth flow');
            expect(def.metadata.flagType).toBe('release');
            expect(def.rules.length).toBe(2);
        }
    });

    it('parses @deprecated annotation', () => {
        const data = `@deprecated "Use FF-new-checkout instead"
@expires 2026-04-01
FF-old-checkout -> true`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-old-checkout')!;
            expect(def.metadata.deprecated).toBe('Use FF-new-checkout instead');
            expect(def.metadata.expires).toBe('2026-04-01');
        }
    });

    it('no metadata is backward compatible', () => {
        const data = 'FF-simple -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-simple')!;
            expect(def.metadata).toEqual({});
        }
    });

    it('parses mixed metadata and no metadata flags', () => {
        const data = `FF-no-meta -> true

@owner "team-a"
FF-with-meta -> false

FF-also-no-meta -> true`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(3);
            expect(r.value.get('FF-no-meta')!.metadata).toEqual({});
            expect(r.value.get('FF-with-meta')!.metadata.owner).toBe('team-a');
            expect(r.value.get('FF-also-no-meta')!.metadata).toEqual({});
        }
    });

    // ── @requires annotation tests ─────────────────────────────────

    it('parses @requires single', () => {
        const data = '@requires FF-new-checkout\nFF-checkout-upsell -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-checkout-upsell')!;
            expect(def.metadata.requires).toEqual(['FF-new-checkout']);
        }
    });

    it('parses @requires multiple', () => {
        const data = '@requires FF-base\n@requires FF-premium\nFF-advanced -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-advanced')!;
            expect(def.metadata.requires).toEqual(['FF-base', 'FF-premium']);
        }
    });

    it('parses @requires with other metadata', () => {
        const data = `@owner "team-a"
@requires FF-base
@type release
FF-feature -> true`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-feature')!;
            expect(def.metadata.owner).toBe('team-a');
            expect(def.metadata.flagType).toBe('release');
            expect(def.metadata.requires).toEqual(['FF-base']);
        }
    });

    it('no @requires is backward compatible', () => {
        const data = 'FF-simple -> true';
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-simple')!;
            expect(def.metadata.requires).toBeUndefined();
        }
    });

    it('parses metadata with comments before it', () => {
        const data = `// A comment about this flag
@owner "devops"
@type ops
FF-ops-flag -> true`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-ops-flag')!;
            expect(def.metadata.owner).toBe('devops');
            expect(def.metadata.flagType).toBe('ops');
        }
    });

    // ── @env rule tests ──────────────────────────────────────────

    it('parses @env simple form', () => {
        const data = `FF-debug {
    @env dev -> true
    @env prod -> false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-debug')!;
            expect(def.rules.length).toBe(2);
            expect(def.rules[0].type).toBe('EnvRule');
            if (def.rules[0].type === 'EnvRule') {
                expect(def.rules[0].env).toBe('dev');
                expect(def.rules[0].rules.length).toBe(1);
            }
            expect(def.rules[1].type).toBe('EnvRule');
            if (def.rules[1].type === 'EnvRule') {
                expect(def.rules[1].env).toBe('prod');
            }
        }
    });

    it('parses @env block form', () => {
        const data = `FF-search {
    @env prod {
        percentage(25%, userId) -> true
        false
    }
    true
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-search')!;
            expect(def.rules.length).toBe(2);
            expect(def.rules[0].type).toBe('EnvRule');
            if (def.rules[0].type === 'EnvRule') {
                expect(def.rules[0].env).toBe('prod');
                expect(def.rules[0].rules.length).toBe(2);
            }
            expect(def.rules[1]).toEqual({
                type: 'Value',
                value: { type: 'OnOff', value: true },
            });
        }
    });

    it('parses @env mixed with regular rules', () => {
        const data = `FF-feature {
    @env dev -> true
    @env stage -> true
    @env prod {
        plan == premium -> true
        false
    }
    false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-feature')!;
            expect(def.rules.length).toBe(4);
            expect(def.rules[0].type).toBe('EnvRule');
            expect(def.rules[1].type).toBe('EnvRule');
            expect(def.rules[2].type).toBe('EnvRule');
            expect(def.rules[3].type).toBe('Value');
        }
    });

    it('parses @env in multi-flag file', () => {
        const data = `FF-simple -> true

FF-env-flag {
    @env dev -> true
    @env prod -> false
}

FF-another -> false`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(3);
            const def = r.value.get('FF-env-flag')!;
            expect(def.rules.length).toBe(2);
        }
    });

    it('parses @env with metadata', () => {
        const data = `@owner "platform-team"
FF-logging {
    @env dev -> true
    false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-logging')!;
            expect(def.metadata.owner).toBe('platform-team');
            expect(def.rules.length).toBe(2);
            expect(def.rules[0].type).toBe('EnvRule');
        }
    });

    // ── Arrow without spaces tests ────────────────────────────────

    it('parses short-form flag without spaces around arrow', () => {
        const r = parseFlagfile('FF-dep-root-new-checkout->true');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            const def = r.value.get('FF-dep-root-new-checkout');
            expect(def).toBeDefined();
            expect(def!.rules.length).toBe(1);
            expect(def!.rules[0]).toEqual({
                type: 'Value',
                value: { type: 'OnOff', value: true },
            });
        }
    });

    it('parses underscore flag name without spaces around arrow', () => {
        const r = parseFlagfile('FF_feature_flag->false');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            const def = r.value.get('FF_feature_flag');
            expect(def).toBeDefined();
            expect(def!.rules[0]).toEqual({
                type: 'Value',
                value: { type: 'OnOff', value: false },
            });
        }
    });

    it('parses @env rule without spaces around arrow', () => {
        const data = `FF-debug {
    @env dev->true
    @env prod->false
}`;
        const r = parseFlagfile(data);
        expect(r.ok).toBe(true);
        if (r.ok) {
            const def = r.value.get('FF-debug')!;
            expect(def.rules.length).toBe(2);
            expect(def.rules[0].type).toBe('EnvRule');
            if (def.rules[0].type === 'EnvRule') {
                expect(def.rules[0].env).toBe('dev');
                expect(def.rules[0].rules[0]).toEqual({
                    type: 'Value',
                    value: { type: 'OnOff', value: true },
                });
            }
            expect(def.rules[1].type).toBe('EnvRule');
            if (def.rules[1].type === 'EnvRule') {
                expect(def.rules[1].env).toBe('prod');
                expect(def.rules[1].rules[0]).toEqual({
                    type: 'Value',
                    value: { type: 'OnOff', value: false },
                });
            }
        }
    });

    it('still parses arrow with spaces (regression)', () => {
        const r = parseFlagfile('FF-feature -> true');
        expect(r.ok).toBe(true);
        if (r.ok) {
            expect(r.value.size).toBe(1);
            const def = r.value.get('FF-feature');
            expect(def).toBeDefined();
            expect(def!.rules[0]).toEqual({
                type: 'Value',
                value: { type: 'OnOff', value: true },
            });
        }
    });
});
