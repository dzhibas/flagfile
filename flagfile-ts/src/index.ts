import { readFileSync } from 'fs';
import {
    Atom,
    FlagReturn,
    Rule,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
} from './ast.js';
import { evaluate, Context } from './eval.js';
import { parseFlagfile } from './flagfile.js';

export {
    Atom,
    AstNode,
    ComparisonOp,
    LogicOp,
    ArrayOp,
    FnCall,
    FlagReturn,
    Rule,
    FlagValue,
    atomString,
    atomNumber,
    atomFloat,
    atomBoolean,
    atomVariable,
    atomDate,
    atomDateTime,
    atomSemver,
    atomEquals,
    atomCompare,
    atomToString,
} from './ast.js';

export { parse, parseAtom, ParseResult } from './parser.js';

export { evaluate, Context } from './eval.js';

export { parseFlagfile } from './flagfile.js';

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

let FLAGS: Map<string, Rule[]> | null = null;

function evaluateRules(
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
 * Parses flagfile content from a string and stores the result in
 * global state. Useful when the content is already in memory.
 *
 * Throws if parsing fails.
 */
export function initFromString(content: string): void {
    if (FLAGS !== null) {
        throw new Error('init() or initFromString() was called more than once');
    }
    const result = parseFlagfile(content);
    if (!result.ok) {
        throw new Error('Failed to parse Flagfile');
    }
    if (result.rest.trim().length > 0) {
        const near = result.rest.trim().split('\n')[0] ?? '';
        throw new Error(
            `Flagfile parsing failed: unexpected content near: ${near}`,
        );
    }
    FLAGS = result.value;
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
    const rules = FLAGS.get(flagName);
    if (!rules) return null;
    const result = evaluateRules(rules, ctx ? toContext(ctx) : {});
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
    const rules = FLAGS.get(flagName);
    if (!rules) return null;
    return evaluateRules(rules, ctx ? toContext(ctx) : {});
}

/**
 * Reset global state. Useful for testing.
 * @internal
 */
export function _reset(): void {
    FLAGS = null;
}
