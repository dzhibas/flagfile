import { readFileSync } from 'fs';
import {
    Atom,
    FlagDefinition,
    FlagMetadata,
    FlagReturn,
    Rule,
    Segments,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
} from './ast.js';
import { evaluate, Context } from './eval.js';
import { parseFlagfile, parseFlagfileWithSegments } from './flagfile.js';

export {
    Atom,
    AstNode,
    ComparisonOp,
    LogicOp,
    ArrayOp,
    MatchOp,
    FnCall,
    FlagReturn,
    FlagMetadata,
    FlagDefinition,
    Rule,
    FlagValue,
    Segments,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
    atomVariable,
    atomDate,
    atomDateTime,
    atomSemver,
    atomRegex,
    atomEquals,
    atomCompare,
    atomToString,
} from './ast.js';

export { parse, parseAtom, ParseResult } from './parser.js';

export { evaluate, Context } from './eval.js';

export { parseFlagfile, parseFlagfileWithSegments, ParsedFlagfile } from './flagfile.js';

// ── Plain-JS context type ───────────────────────────────────────────

/** Plain JS context: values are auto-coerced to Atoms internally. */
export type SimpleContext = Record<string, string | number | boolean>;

// ── Internal helpers ────────────────────────────────────────────────

function toContext(ctx: SimpleContext): Context {
    const out: Context = {};
    for (const key of Object.keys(ctx)) {
        const v = ctx[key];
        if (typeof v === 'string') {
            out[key] = atomString(v);
        } else if (typeof v === 'boolean') {
            out[key] = atomBoolean(v);
        } else if (typeof v === 'number') {
            out[key] = Number.isInteger(v) ? atomNumber(v) : atomFloat(v);
        }
    }
    return out;
}

function unwrap(val: FlagReturn): boolean | number | string | unknown {
    return val.value;
}

// ── Global flag state ───────────────────────────────────────────────

let FLAGS: Map<string, FlagDefinition> | null = null;
let SEGMENTS: Segments = new Map();
let ENV: string | null = null;

function evaluateRules(
    rules: Rule[],
    context: Context,
    flagName?: string,
    segments?: Segments,
): FlagReturn | null {
    const segs = segments ?? SEGMENTS;
    for (const rule of rules) {
        if (rule.type === 'Value') {
            return rule.value;
        }
        if (rule.type === 'BoolExpressionValue') {
            if (evaluate(rule.expr, context, flagName, segs)) {
                return rule.value;
            }
        }
        if (rule.type === 'EnvRule') {
            if (ENV !== null && ENV === rule.env) {
                const result = evaluateRules(rule.rules, context, flagName, segs);
                if (result !== null) {
                    return result;
                }
            }
        }
    }
    return null;
}

/**
 * Reads and parses a `Flagfile` from the current directory, storing
 * the result in global state for later use with {@link ff}.
 *
 * Throws if the file cannot be read or parsed.
 */
export function init(): void {
    const content = readFileSync('Flagfile', 'utf-8');
    initFromString(content);
}

/**
 * Like {@link init}, but also sets the current environment for `@env` rules.
 */
export function initWithEnv(env: string): void {
    ENV = env;
    init();
}

/**
 * Like {@link initFromString}, but also sets the current environment
 * for `@env` rules.
 */
export function initFromStringWithEnv(content: string, env: string): void {
    ENV = env;
    initFromString(content);
}

/**
 * Parses flagfile content from a string and stores the result in
 * global state. Useful when the content is already in memory.
 *
 * Throws if parsing fails.
 */
export function initFromString(content: string): void {
    if (FLAGS !== null) {
        throw new Error('init() or initFromString() was called more than once');
    }
    const result = parseFlagfileWithSegments(content);
    if (!result.ok) {
        throw new Error('Failed to parse Flagfile');
    }
    if (result.rest.trim().length > 0) {
        const near = result.rest.trim().split('\n')[0] ?? '';
        throw new Error(
            `Flagfile parsing failed: unexpected content near: ${near}`,
        );
    }
    FLAGS = result.value.flags;
    SEGMENTS = result.value.segments;
}

/**
 * Evaluates a flag by name and returns the unwrapped JS value.
 *
 * - `OnOff`   → `boolean`
 * - `Integer` → `number`
 * - `Str`     → `string`
 * - `Json`    → parsed object
 *
 * Returns `null` if the flag was not found or no rule matched.
 * Context is optional — omit it for flags with no conditions.
 *
 * Throws if {@link init} or {@link initFromString} has not been called.
 */
export function ff(
    flagName: string,
    ctx?: SimpleContext,
): boolean | number | string | unknown | null {
    if (FLAGS === null) {
        throw new Error('init() or initFromString() must be called before ff()');
    }
    const def = FLAGS.get(flagName);
    if (!def) return null;
    const context = ctx ? toContext(ctx) : {};

    // Check @requires prerequisites
    if (def.metadata.requires && def.metadata.requires.length > 0) {
        for (const req of def.metadata.requires) {
            const reqDef = FLAGS.get(req);
            if (!reqDef) return null; // required flag doesn't exist
            const reqResult = evaluateRules(reqDef.rules, context, req);
            if (!reqResult || reqResult.type !== 'OnOff' || !reqResult.value) {
                return null; // prerequisite not met
            }
        }
    }

    const result = evaluateRules(def.rules, context, flagName);
    return result ? unwrap(result) : null;
}

/**
 * Like {@link ff} but returns the raw `FlagReturn` discriminated union
 * instead of unwrapping the value. Useful when you need to inspect the
 * flag type (OnOff, Integer, Str, Json).
 *
 * Returns `null` if the flag was not found or no rule matched.
 * Context is optional.
 */
export function ffRaw(
    flagName: string,
    ctx?: SimpleContext,
): FlagReturn | null {
    if (FLAGS === null) {
        throw new Error('init() or initFromString() must be called before ffRaw()');
    }
    const def = FLAGS.get(flagName);
    if (!def) return null;
    const context = ctx ? toContext(ctx) : {};

    // Check @requires prerequisites
    if (def.metadata.requires && def.metadata.requires.length > 0) {
        for (const req of def.metadata.requires) {
            const reqDef = FLAGS.get(req);
            if (!reqDef) return null;
            const reqResult = evaluateRules(reqDef.rules, context, req);
            if (!reqResult || reqResult.type !== 'OnOff' || !reqResult.value) {
                return null;
            }
        }
    }

    return evaluateRules(def.rules, context, flagName);
}

/**
 * Returns the metadata annotations for a flag.
 *
 * Returns `null` if the flag was not found.
 *
 * Throws if {@link init} or {@link initFromString} has not been called.
 */
export function ffMetadata(flagName: string): FlagMetadata | null {
    if (FLAGS === null) {
        throw new Error('init() or initFromString() must be called before ffMetadata()');
    }
    const def = FLAGS.get(flagName);
    if (!def) return null;
    return def.metadata;
}

/**
 * Reset global state. Useful for testing.
 * @internal
 */
export function _reset(): void {
    FLAGS = null;
    SEGMENTS = new Map();
    ENV = null;
}
