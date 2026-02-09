// ── Atom: leaf values in expressions ────────────────────────────────

export type Atom =
    | { type: 'String'; value: string }
    | { type: 'Number'; value: number }      // integer (i32)
    | { type: 'Float'; value: number }        // f64
    | { type: 'Boolean'; value: boolean }
    | { type: 'Variable'; value: string }
    | { type: 'Date'; value: string }         // YYYY-MM-DD
    | { type: 'DateTime'; value: string }
    | { type: 'Semver'; major: number; minor: number; patch: number };

// ── Operators ──────────────────────────────────────────────────────

export enum ComparisonOp {
    Eq = 'Eq',
    NotEq = 'NotEq',
    More = 'More',
    MoreEq = 'MoreEq',
    Less = 'Less',
    LessEq = 'LessEq',
}

export enum LogicOp {
    And = 'And',
    Or = 'Or',
}

export enum ArrayOp {
    In = 'In',
    NotIn = 'NotIn',
}

export enum FnCall {
    Upper = 'Upper',
    Lower = 'Lower',
    Now = 'Now',
}

// ── AST nodes ──────────────────────────────────────────────────────

export type AstNode =
    | { type: 'Void' }
    | { type: 'Variable'; atom: Atom }
    | { type: 'Function'; fn: FnCall; arg: AstNode }
    | { type: 'Constant'; atom: Atom }
    | { type: 'List'; items: Atom[] }
    | { type: 'Compare'; left: AstNode; op: ComparisonOp; right: AstNode }
    | { type: 'Array'; left: AstNode; op: ArrayOp; right: AstNode }
    | { type: 'Logic'; left: AstNode; op: LogicOp; right: AstNode }
    | { type: 'Scope'; expr: AstNode; negate: boolean };

// ── Flag return values ─────────────────────────────────────────────

export type FlagReturn =
    | { type: 'OnOff'; value: boolean }
    | { type: 'Json'; value: unknown }
    | { type: 'Integer'; value: number }
    | { type: 'Str'; value: string };

// ── Rules ──────────────────────────────────────────────────────────

export type Rule =
    | { type: 'Value'; value: FlagReturn }
    | { type: 'BoolExpressionValue'; expr: AstNode; value: FlagReturn };

// ── Parsed flag map ────────────────────────────────────────────────

export type FlagValue = Map<string, Rule[]>;

// ── Helper constructors ────────────────────────────────────────────

export function atomString(v: string): Atom {
    return { type: 'String', value: v };
}
export function atomNumber(v: number): Atom {
    return { type: 'Number', value: v };
}
export function atomFloat(v: number): Atom {
    return { type: 'Float', value: v };
}
export function atomBoolean(v: boolean): Atom {
    return { type: 'Boolean', value: v };
}
export function atomVariable(v: string): Atom {
    return { type: 'Variable', value: v };
}
export function atomDate(v: string): Atom {
    return { type: 'Date', value: v };
}
export function atomDateTime(v: string): Atom {
    return { type: 'DateTime', value: v };
}
export function atomSemver(major: number, minor: number, patch: number): Atom {
    return { type: 'Semver', major, minor, patch };
}

// ── Atom comparison helpers (mirrors Rust PartialEq / PartialOrd) ──

function floatToSemver(f: number): [number, number, number] | null {
    const s = String(f);
    const dot = s.indexOf('.');
    if (dot === -1) {
        const maj = Math.floor(f);
        if (maj < 0 || !Number.isInteger(f)) return null;
        return [maj, 0, 0];
    }
    const majStr = s.slice(0, dot);
    const minStr = s.slice(dot + 1);
    const maj = parseInt(majStr, 10);
    const min = parseInt(minStr, 10);
    if (isNaN(maj) || isNaN(min)) return null;
    return [maj, min, 0];
}

function atomToNumber(a: Atom): number | null {
    if (a.type === 'Number') return a.value;
    if (a.type === 'Float') return a.value;
    return null;
}

export function atomEquals(a: Atom, b: Atom): boolean {
    // Same type comparisons
    if (a.type === 'String' && b.type === 'String') return a.value === b.value;
    if (a.type === 'Variable' && b.type === 'Variable') return a.value === b.value;
    if (a.type === 'String' && b.type === 'Variable') return a.value === b.value;
    if (a.type === 'Variable' && b.type === 'String') return a.value === b.value;
    if (a.type === 'Number' && b.type === 'Number') return a.value === b.value;
    if (a.type === 'Float' && b.type === 'Float') return a.value === b.value;
    if (a.type === 'Boolean' && b.type === 'Boolean') return a.value === b.value;
    if (a.type === 'Date' && b.type === 'Date') return a.value === b.value;
    if (a.type === 'DateTime' && b.type === 'DateTime') return a.value === b.value;

    // Semver comparisons
    if (a.type === 'Semver' && b.type === 'Semver') {
        return a.major === b.major && a.minor === b.minor && a.patch === b.patch;
    }

    // Semver ↔ Float
    if (a.type === 'Semver' && b.type === 'Float') {
        const sv = floatToSemver(b.value);
        return sv !== null && a.major === sv[0] && a.minor === sv[1] && a.patch === sv[2];
    }
    if (a.type === 'Float' && b.type === 'Semver') {
        const sv = floatToSemver(a.value);
        return sv !== null && b.major === sv[0] && b.minor === sv[1] && b.patch === sv[2];
    }

    // Semver ↔ Number
    if (a.type === 'Semver' && b.type === 'Number') {
        return b.value >= 0 && a.major === b.value && a.minor === 0 && a.patch === 0;
    }
    if (a.type === 'Number' && b.type === 'Semver') {
        return a.value >= 0 && b.major === a.value && b.minor === 0 && b.patch === 0;
    }

    return false;
}

/**
 * Returns negative if a < b, 0 if equal, positive if a > b, or null if
 * the two atoms are not comparable.
 */
export function atomCompare(a: Atom, b: Atom): number | null {
    // Number ↔ Number
    if (a.type === 'Number' && b.type === 'Number') return a.value - b.value;
    // Number ↔ Float
    if (a.type === 'Number' && b.type === 'Float') return a.value - b.value;
    if (a.type === 'Float' && b.type === 'Number') return a.value - b.value;
    // Float ↔ Float
    if (a.type === 'Float' && b.type === 'Float') return a.value - b.value;

    // Date ↔ Date (lexicographic works for YYYY-MM-DD)
    if (a.type === 'Date' && b.type === 'Date') {
        if (a.value < b.value) return -1;
        if (a.value > b.value) return 1;
        return 0;
    }

    // Semver ↔ Semver
    if (a.type === 'Semver' && b.type === 'Semver') {
        const cmp = (a.major - b.major) || (a.minor - b.minor) || (a.patch - b.patch);
        return cmp;
    }

    // Semver ↔ Float
    if (a.type === 'Semver' && b.type === 'Float') {
        const sv = floatToSemver(b.value);
        if (sv === null) return null;
        return (a.major - sv[0]) || (a.minor - sv[1]) || (a.patch - sv[2]);
    }
    if (a.type === 'Float' && b.type === 'Semver') {
        const sv = floatToSemver(a.value);
        if (sv === null) return null;
        return (sv[0] - b.major) || (sv[1] - b.minor) || (sv[2] - b.patch);
    }

    // Semver ↔ Number
    if (a.type === 'Semver' && b.type === 'Number') {
        if (b.value < 0) return null;
        return (a.major - b.value) || (a.minor - 0) || (a.patch - 0);
    }
    if (a.type === 'Number' && b.type === 'Semver') {
        if (a.value < 0) return null;
        return (a.value - b.major) || (0 - b.minor) || (0 - b.patch);
    }

    return null;
}

export function atomToString(a: Atom): string {
    switch (a.type) {
        case 'String': return a.value;
        case 'Number': return String(a.value);
        case 'Float': return String(a.value);
        case 'Boolean': return String(a.value);
        case 'Variable': return a.value;
        case 'Date': return a.value;
        case 'DateTime': return a.value;
        case 'Semver': return `${a.major}.${a.minor}.${a.patch}`;
    }
}
