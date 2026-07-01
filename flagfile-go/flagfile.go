package flagfile

import (
	"encoding/json"
	"reflect"
	"strconv"
	"strings"
	"time"
)

// ── Return values, rules, metadata ───────────────────────────

// FRKind identifies the kind of a flag's return value.
type FRKind int

const (
	FROnOff FRKind = iota
	FRJson
	FRInt
	FRStr
)

// FlagReturn mirrors the Rust `FlagReturn` enum.
type FlagReturn struct {
	Kind FRKind
	Bool bool
	Json interface{}
	Int  int64
	Str  string
}

// RuleKind identifies the kind of a rule.
type RuleKind int

const (
	RuleValue RuleKind = iota // bare fallthrough value
	RuleExpr                  // condition -> value
	RuleEnv                   // @env block
)

// Rule mirrors the Rust `Rule` enum.
type Rule struct {
	Kind     RuleKind
	Value    FlagReturn // RuleValue
	Expr     *AstNode   // RuleExpr
	Return   FlagReturn // RuleExpr
	Name     string     // RuleExpr (optional @name)
	Env      string     // RuleEnv
	SubRules []Rule     // RuleEnv
}

// FlagMetadata mirrors the Rust `FlagMetadata` struct.
type FlagMetadata struct {
	Owner          string
	HasOwner       bool
	Expires        time.Time
	HasExpires     bool
	Ticket         string
	HasTicket      bool
	Description    string
	HasDescription bool
	FlagType       string
	HasFlagType    bool
	Deprecated     string
	HasDeprecated  bool
	Requires       []string
	Tests          []string
}

// FlagDefinition mirrors the Rust `FlagDefinition`.
type FlagDefinition struct {
	Rules    []Rule
	Metadata FlagMetadata
}

// FlagEntry is an ordered (name, definition) pair.
type FlagEntry struct {
	Name string
	Def  FlagDefinition
}

// ParsedFlagfile holds ordered flags and named segments.
type ParsedFlagfile struct {
	Flags    []FlagEntry
	Segments Segments
}

// ── Comment helpers ──────────────────────────────────────────

func parseComment(i string) (string, bool) {
	r := skipWs(i)
	if !strings.HasPrefix(r, "//") {
		return i, false
	}
	r = r[2:]
	r = skipWs(r)
	j := 0
	for j < len(r) && r[j] != '\n' && r[j] != '\r' {
		j++
	}
	if j == 0 {
		return i, false
	}
	return r[j:], true
}

func parseMultilineComment(i string) (string, bool) {
	if !strings.HasPrefix(i, "/*") {
		return i, false
	}
	idx := strings.Index(i[2:], "*/")
	if idx < 0 {
		return i, false
	}
	return i[2+idx+2:], true
}

func many0Comments(i string) string {
	for {
		if r, ok := parseComment(i); ok && len(r) < len(i) {
			i = r
			continue
		}
		if r, ok := parseMultilineComment(i); ok && len(r) < len(i) {
			i = r
			continue
		}
		break
	}
	return i
}

// ── Return value parsers ─────────────────────────────────────

func parseBoolReturn(i string) (string, FlagReturn, bool) {
	if r, a, ok := parseBoolean(i); ok {
		return r, FlagReturn{Kind: FROnOff, Bool: a.Bool}, true
	}
	return i, FlagReturn{}, false
}

func parseJsonReturn(i string) (string, FlagReturn, bool) {
	if !strings.HasPrefix(i, "json(") {
		return i, FlagReturn{}, false
	}
	r := i[len("json("):]
	idx := strings.IndexByte(r, ')')
	if idx < 0 {
		return i, FlagReturn{}, false
	}
	content := r[:idx]
	r = r[idx:]
	r = skipWs(r)
	if !strings.HasPrefix(r, ")") {
		return i, FlagReturn{}, false
	}
	r = r[1:]
	var v interface{}
	if err := json.Unmarshal([]byte(content), &v); err != nil {
		return i, FlagReturn{}, false
	}
	return r, FlagReturn{Kind: FRJson, Json: v}, true
}

func parseStringReturn(i string) (string, FlagReturn, bool) {
	if len(i) > 0 && i[0] == '"' {
		idx := strings.IndexByte(i[1:], '"')
		if idx < 0 {
			return i, FlagReturn{}, false
		}
		return i[1+idx+1:], FlagReturn{Kind: FRStr, Str: i[1 : 1+idx]}, true
	}
	if len(i) > 0 && i[0] == '\'' {
		idx := strings.IndexByte(i[1:], '\'')
		if idx < 0 {
			return i, FlagReturn{}, false
		}
		return i[1+idx+1:], FlagReturn{Kind: FRStr, Str: i[1 : 1+idx]}, true
	}
	return i, FlagReturn{}, false
}

func parseIntReturn(i string) (string, FlagReturn, bool) {
	orig := i
	j := 0
	if j < len(i) && i[j] == '-' {
		j++
	}
	start := j
	for j < len(i) && isDigit(i[j]) {
		j++
	}
	if j == start {
		return orig, FlagReturn{}, false
	}
	n, err := strconv.ParseInt(i[:j], 10, 64)
	if err != nil {
		return orig, FlagReturn{}, false
	}
	return i[j:], FlagReturn{Kind: FRInt, Int: n}, true
}

func parseReturnVal(i string) (string, FlagReturn, bool) {
	for _, p := range []func(string) (string, FlagReturn, bool){
		parseBoolReturn,
		parseJsonReturn,
		parseStringReturn,
		parseIntReturn,
	} {
		r := skipWs(i)
		if r2, v, ok := p(r); ok {
			return skipWs(r2), v, true
		}
	}
	return i, FlagReturn{}, false
}

// ── Annotation parsing ───────────────────────────────────────

type annKind int

const (
	annOwner annKind = iota
	annExpires
	annTicket
	annDescription
	annType
	annDeprecated
	annRequires
	annTest
)

type annotation struct {
	kind annKind
	str  string
	date time.Time
}

func skipSpaces(i string) string {
	j := 0
	for j < len(i) && (i[j] == ' ' || i[j] == '\t') {
		j++
	}
	return i[j:]
}

func takeTillNewline(i string) (string, string) {
	j := 0
	for j < len(i) && i[j] != '\n' && i[j] != '\r' {
		j++
	}
	return i[:j], i[j:]
}

func parseQuotedString(i string) (string, string, bool) {
	if len(i) > 0 && i[0] == '"' {
		idx := strings.IndexByte(i[1:], '"')
		if idx < 0 {
			return i, "", false
		}
		return i[1+idx+1:], i[1 : 1+idx], true
	}
	if len(i) > 0 && i[0] == '\'' {
		idx := strings.IndexByte(i[1:], '\'')
		if idx < 0 {
			return i, "", false
		}
		return i[1+idx+1:], i[1 : 1+idx], true
	}
	return i, "", false
}

// tagWs matches a case-sensitive tag with surrounding whitespace stripping.
func tagWs(i, t string) (string, bool) {
	r := skipWs(i)
	if !strings.HasPrefix(r, t) {
		return i, false
	}
	r = r[len(t):]
	return skipWs(r), true
}

func wsQuoted(i string) (string, string, bool) {
	r := skipWs(i)
	r, s, ok := parseQuotedString(r)
	if !ok {
		return i, "", false
	}
	return skipWs(r), s, true
}

func parseAnnotationOwner(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@owner")
	if !ok {
		return i, annotation{}, false
	}
	r, s, ok := wsQuoted(r)
	if !ok {
		return i, annotation{}, false
	}
	return r, annotation{kind: annOwner, str: s}, true
}

func parseAnnotationExpires(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@expires")
	if !ok {
		return i, annotation{}, false
	}
	r = skipWs(r)
	matched, rest, ok := matchDatePattern(r)
	if !ok {
		return i, annotation{}, false
	}
	t, err := time.Parse("2006-01-02", matched)
	if err != nil {
		return i, annotation{}, false
	}
	rest = skipWs(rest)
	return rest, annotation{kind: annExpires, date: t.UTC()}, true
}

func parseAnnotationTicket(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@ticket")
	if !ok {
		return i, annotation{}, false
	}
	r, s, ok := wsQuoted(r)
	if !ok {
		return i, annotation{}, false
	}
	return r, annotation{kind: annTicket, str: s}, true
}

func parseStringOrLine(i string) (string, string, bool) {
	r := skipSpaces(i)
	if r2, s, ok := parseQuotedString(r); ok {
		return r2, s, true
	}
	val, rest := takeTillNewline(r)
	val = strings.TrimSpace(val)
	if val == "" {
		return i, "", false
	}
	return rest, val, true
}

func parseAnnotationDescription(i string) (string, annotation, bool) {
	r := skipWs(i)
	if !strings.HasPrefix(r, "@description") {
		return i, annotation{}, false
	}
	r = r[len("@description"):]
	r, s, ok := parseStringOrLine(r)
	if !ok {
		return i, annotation{}, false
	}
	return r, annotation{kind: annDescription, str: s}, true
}

func parseAnnotationType(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@type")
	if !ok {
		return i, annotation{}, false
	}
	r = skipWs(r)
	j := 0
	for j < len(r) && (isAlnum(r[j]) || r[j] == '-' || r[j] == '_') {
		j++
	}
	if j == 0 {
		return i, annotation{}, false
	}
	val := r[:j]
	r = skipWs(r[j:])
	return r, annotation{kind: annType, str: val}, true
}

func parseAnnotationDeprecated(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@deprecated")
	if !ok {
		return i, annotation{}, false
	}
	r, s, ok := wsQuoted(r)
	if !ok {
		return i, annotation{}, false
	}
	return r, annotation{kind: annDeprecated, str: s}, true
}

func parseAnnotationRequires(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@requires")
	if !ok {
		return i, annotation{}, false
	}
	r = skipWs(r)
	r, name, ok := parseFlagName(r)
	if !ok {
		return i, annotation{}, false
	}
	r = skipWs(r)
	return r, annotation{kind: annRequires, str: name}, true
}

func parseAnnotationTest(i string) (string, annotation, bool) {
	r, ok := tagWs(i, "@test")
	if !ok {
		return i, annotation{}, false
	}
	r = skipWs(r)
	val, rest := takeTillNewline(r)
	val = strings.TrimSpace(val)
	if val == "" {
		return i, annotation{}, false
	}
	return rest, annotation{kind: annTest, str: val}, true
}

func parseAnnotation(i string) (string, annotation, bool) {
	for _, p := range []func(string) (string, annotation, bool){
		parseAnnotationOwner,
		parseAnnotationExpires,
		parseAnnotationTicket,
		parseAnnotationDescription,
		parseAnnotationType,
		parseAnnotationDeprecated,
		parseAnnotationRequires,
		parseAnnotationTest,
	} {
		if r, a, ok := p(i); ok {
			return r, a, true
		}
	}
	return i, annotation{}, false
}

func parseMetadataBlock(i string) (string, FlagMetadata) {
	var meta FlagMetadata
	for {
		r := many0Comments(i)
		r2, ann, ok := parseAnnotation(r)
		if !ok {
			break
		}
		switch ann.kind {
		case annOwner:
			meta.Owner, meta.HasOwner = ann.str, true
		case annExpires:
			meta.Expires, meta.HasExpires = ann.date, true
		case annTicket:
			meta.Ticket, meta.HasTicket = ann.str, true
		case annDescription:
			meta.Description, meta.HasDescription = ann.str, true
		case annType:
			meta.FlagType, meta.HasFlagType = ann.str, true
		case annDeprecated:
			meta.Deprecated, meta.HasDeprecated = ann.str, true
		case annRequires:
			meta.Requires = append(meta.Requires, ann.str)
		case annTest:
			meta.Tests = append(meta.Tests, ann.str)
		}
		if len(r2) == len(i) {
			break
		}
		i = r2
	}
	return i, meta
}

// ── Flag name parsing ────────────────────────────────────────

func parseFlagName(i string) (string, string, bool) {
	var prefix string
	if strings.HasPrefix(i, "FF-") {
		prefix = "FF-"
	} else if strings.HasPrefix(i, "FF_") {
		prefix = "FF_"
	} else {
		return i, "", false
	}
	j := len(prefix)
	for j < len(i) {
		c := i[j]
		if isAlnum(c) || c == '_' {
			j++
			continue
		}
		if c == '-' && (j+1 >= len(i) || i[j+1] != '>') {
			j++
			continue
		}
		break
	}
	return i[j:], i[:j], true
}

func parseEnvName(i string) (string, string, bool) {
	j := 0
	for j < len(i) {
		c := i[j]
		if isAlnum(c) || c == '_' {
			j++
			continue
		}
		if c == '-' && (j+1 >= len(i) || i[j+1] != '>') {
			j++
			continue
		}
		break
	}
	if j == 0 {
		return i, "", false
	}
	return i[j:], i[:j], true
}

// ── Rule parsing ─────────────────────────────────────────────

func parseAnonymousFunc(i string) (string, FlagEntry, bool) {
	orig := i
	r := skipWs(i)
	r, name, ok := parseFlagName(r)
	if !ok {
		return orig, FlagEntry{}, false
	}
	r = skipWs(r)
	r, ok = tagWsExact(r, "->")
	if !ok {
		return orig, FlagEntry{}, false
	}
	r, val, ok := parseReturnVal(r)
	if !ok {
		return orig, FlagEntry{}, false
	}
	return r, FlagEntry{Name: name, Def: FlagDefinition{Rules: []Rule{{Kind: RuleValue, Value: val}}}}, true
}

// tagWsExact matches a tag after skipping leading whitespace (no trailing skip).
func tagWsExact(i, t string) (string, bool) {
	r := skipWs(i)
	if !strings.HasPrefix(r, t) {
		return i, false
	}
	return r[len(t):], true
}

func parseRuleExpr(i string) (string, Rule, bool) {
	orig := i
	r, expr, ok := Parse(i)
	if !ok {
		return orig, Rule{}, false
	}
	r = skipWs(r)
	r, ok = tagWsExact(r, "->")
	if !ok {
		return orig, Rule{}, false
	}
	r, val, ok := parseReturnVal(r)
	if !ok {
		return orig, Rule{}, false
	}
	return r, Rule{Kind: RuleExpr, Expr: nodePtr(expr), Return: val}, true
}

func parseRuleStatic(i string) (string, Rule, bool) {
	r, val, ok := parseReturnVal(i)
	if !ok {
		return i, Rule{}, false
	}
	return r, Rule{Kind: RuleValue, Value: val}, true
}

func parseEnvRuleSimple(i string) (string, Rule, bool) {
	orig := i
	r, ok := tagWs(i, "@env")
	if !ok {
		return orig, Rule{}, false
	}
	r = skipWs(r)
	r, env, ok := parseEnvName(r)
	if !ok {
		return orig, Rule{}, false
	}
	r = skipWs(r)
	r, ok = tagWsExact(r, "->")
	if !ok {
		return orig, Rule{}, false
	}
	r, val, ok := parseReturnVal(r)
	if !ok {
		return orig, Rule{}, false
	}
	return r, Rule{Kind: RuleEnv, Env: env, SubRules: []Rule{{Kind: RuleValue, Value: val}}}, true
}

func parseEnvRuleBlock(i string) (string, Rule, bool) {
	orig := i
	r, ok := tagWs(i, "@env")
	if !ok {
		return orig, Rule{}, false
	}
	r = skipWs(r)
	r, env, ok := parseEnvName(r)
	if !ok {
		return orig, Rule{}, false
	}
	r, ok = tagWs(r, "{")
	if !ok {
		return orig, Rule{}, false
	}
	r, rules, ok := parseRulesList(r)
	if !ok {
		return orig, Rule{}, false
	}
	r, ok = tagWs(r, "}")
	if !ok {
		return orig, Rule{}, false
	}
	return r, Rule{Kind: RuleEnv, Env: env, SubRules: rules}, true
}

func parseEnvRule(i string) (string, Rule, bool) {
	if r, rule, ok := parseEnvRuleBlock(i); ok {
		return r, rule, true
	}
	return parseEnvRuleSimple(i)
}

func parseRules(i string) (string, Rule, bool) {
	if r, rule, ok := parseEnvRule(i); ok {
		return r, rule, true
	}
	if r, rule, ok := parseRuleExpr(i); ok {
		return r, rule, true
	}
	return parseRuleStatic(i)
}

func parseRuleName(i string) (string, string, bool) {
	r := skipWs(i)
	if strings.HasPrefix(r, "//") {
		r = r[2:]
	}
	r2, ok := tagWs(r, "@name")
	if !ok {
		return i, "", false
	}
	r = r2
	if r3, s, ok := wsQuoted(r); ok {
		return r3, s, true
	}
	val, rest := takeTillNewline(r)
	return rest, strings.TrimSpace(val), true
}

func parseRulePrefix(i string) (string, string) {
	name := ""
	for {
		if r, n, ok := parseRuleName(i); ok && len(r) <= len(i) {
			name = n
			if len(r) == len(i) {
				break
			}
			i = r
			continue
		}
		if r, ok := parseComment(i); ok && len(r) < len(i) {
			i = r
			continue
		}
		if r, ok := parseMultilineComment(i); ok && len(r) < len(i) {
			i = r
			continue
		}
		break
	}
	return i, name
}

func parseRulesOrComments(i string) (string, Rule, bool) {
	r, name := parseRulePrefix(i)
	r2, rule, ok := parseRules(r)
	if !ok {
		return i, Rule{}, false
	}
	if rule.Kind == RuleExpr && name != "" {
		rule.Name = name
	}
	return r2, rule, true
}

func parseRulesList(i string) (string, []Rule, bool) {
	r, rule, ok := parseRulesOrComments(i)
	if !ok {
		return i, nil, false
	}
	rules := []Rule{rule}
	i = r
	for {
		r2, rule2, ok2 := parseRulesOrComments(i)
		if !ok2 || len(r2) >= len(i) {
			break
		}
		rules = append(rules, rule2)
		i = r2
	}
	i = many0Comments(i)
	return i, rules, true
}

func parseFunction(i string) (string, FlagEntry, bool) {
	orig := i
	r := skipWs(i)
	r, name, ok := parseFlagName(r)
	if !ok {
		return orig, FlagEntry{}, false
	}
	r, ok = tagWs(r, "{")
	if !ok {
		return orig, FlagEntry{}, false
	}
	r, rules, ok := parseRulesList(r)
	if !ok {
		return orig, FlagEntry{}, false
	}
	r, ok = tagWs(r, "}")
	if !ok {
		return orig, FlagEntry{}, false
	}
	return r, FlagEntry{Name: name, Def: FlagDefinition{Rules: rules}}, true
}

func parseFlagEntry(i string) (string, FlagEntry, bool) {
	orig := i
	r := many0Comments(i)
	r, meta := parseMetadataBlock(r)
	r = many0Comments(r)
	entry, ok := FlagEntry{}, false
	if r2, e, o := parseAnonymousFunc(r); o {
		r, entry, ok = r2, e, true
	} else if r2, e, o := parseFunction(r); o {
		r, entry, ok = r2, e, true
	}
	if !ok {
		return orig, FlagEntry{}, false
	}
	entry.Def.Metadata = meta
	r = many0Comments(r)
	return r, entry, true
}

func parseSegmentDefinition(i string) (string, string, *AstNode, bool) {
	orig := i
	r := many0Comments(i)
	r, ok := tagWs(r, "@segment")
	if !ok {
		return orig, "", nil, false
	}
	r = skipWs(r)
	r, name, ok := parseSegmentName(r)
	if !ok {
		return orig, "", nil, false
	}
	r = skipWs(r)
	r, ok = tagWs(r, "{")
	if !ok {
		return orig, "", nil, false
	}
	r, expr, ok := Parse(r)
	if !ok {
		return orig, "", nil, false
	}
	r, ok = tagWs(r, "}")
	if !ok {
		return orig, "", nil, false
	}
	r = many0Comments(r)
	return r, name, nodePtr(expr), true
}

// ParseFlagfileWithSegments parses a full Flagfile. Returns remaining input.
func ParseFlagfileWithSegments(i string) (string, ParsedFlagfile) {
	parsed := ParsedFlagfile{Segments: Segments{}}
	for {
		if r, name, expr, ok := parseSegmentDefinition(i); ok && len(r) < len(i) {
			parsed.Segments[name] = expr
			i = r
			continue
		}
		if r, entry, ok := parseFlagEntry(i); ok && len(r) < len(i) {
			parsed.Flags = append(parsed.Flags, entry)
			i = r
			continue
		}
		break
	}
	return i, parsed
}

// ── @test extraction (line scan) ─────────────────────────────

// TestAnnotation mirrors the Rust struct.
type TestAnnotation struct {
	Assertion  string
	LineNumber int // 1-based
}

func extractTestFromCommentLine(line string) (string, bool) {
	trimmed := strings.TrimSpace(line)
	trimmed = strings.TrimLeft(trimmed, "*")
	trimmed = strings.TrimSpace(trimmed)
	if rest, ok := strings.CutPrefix(trimmed, "@test "); ok {
		assertion := strings.TrimSpace(rest)
		if assertion != "" {
			return assertion, true
		}
	}
	return "", false
}

// ExtractTestAnnotations scans raw content for @test in // and /* */ comments.
func ExtractTestAnnotations(content string) []TestAnnotation {
	var results []TestAnnotation
	inBlock := false
	lines := strings.Split(content, "\n")
	for idx, line := range lines {
		line = strings.TrimSuffix(line, "\r")
		lineNumber := idx + 1

		if inBlock {
			if pos := strings.Index(line, "*/"); pos >= 0 {
				before := line[:pos]
				if a, ok := extractTestFromCommentLine(before); ok {
					results = append(results, TestAnnotation{a, lineNumber})
				}
				inBlock = false
			} else if a, ok := extractTestFromCommentLine(line); ok {
				results = append(results, TestAnnotation{a, lineNumber})
			}
			continue
		}

		if pos := strings.Index(line, "//"); pos >= 0 {
			body := line[pos+2:]
			if a, ok := extractTestFromCommentLine(body); ok {
				results = append(results, TestAnnotation{a, lineNumber})
			}
		}

		if start := strings.Index(line, "/*"); start >= 0 {
			if end := strings.Index(line[start+2:], "*/"); end >= 0 {
				body := line[start+2 : start+2+end]
				if a, ok := extractTestFromCommentLine(body); ok {
					results = append(results, TestAnnotation{a, lineNumber})
				}
			} else {
				after := line[start+2:]
				if a, ok := extractTestFromCommentLine(after); ok {
					results = append(results, TestAnnotation{a, lineNumber})
				}
				inBlock = true
			}
		}
	}
	return results
}

// ── Evaluation of a flag's rules ─────────────────────────────

// EvaluateRules mirrors Rust `evaluate_rules_with_env`.
func EvaluateRules(rules []Rule, ctx Context, flagName string, segments Segments, env string, hasEnv bool) (FlagReturn, bool) {
	for _, rule := range rules {
		switch rule.Kind {
		case RuleExpr:
			if EvalWithSegments(rule.Expr, ctx, flagName, segments) {
				return rule.Return, true
			}
		case RuleValue:
			return rule.Value, true
		case RuleEnv:
			if hasEnv && env == rule.Env {
				if res, ok := EvaluateRules(rule.SubRules, ctx, flagName, segments, env, hasEnv); ok {
					return res, true
				}
			}
		}
	}
	return FlagReturn{}, false
}

// EvaluateFlag mirrors Rust `evaluate_flag_with_env`, checking @requires.
func EvaluateFlag(flagName string, ctx Context, flags map[string]FlagDefinition, segments Segments, env string, hasEnv bool) (FlagReturn, bool) {
	if def, ok := flags[flagName]; ok {
		for _, req := range def.Metadata.Requires {
			reqDef, exists := flags[req]
			if !exists {
				return FlagReturn{}, false
			}
			res, ok := EvaluateRules(reqDef.Rules, ctx, req, segments, env, hasEnv)
			if !ok || res.Kind != FROnOff || !res.Bool {
				return FlagReturn{}, false
			}
		}
	}
	def, ok := flags[flagName]
	if !ok {
		return FlagReturn{}, false
	}
	return EvaluateRules(def.Rules, ctx, flagName, segments, env, hasEnv)
}

// ── Test line parsing / assertion matching (CLI) ─────────────

// ParseTestLine parses `FF-name(k=v,...) == EXPECTED` or `FF-name == EXPECTED`.
func ParseTestLine(line string) (string, [][2]string, string, bool) {
	line = strings.TrimSpace(line)
	if line == "" {
		return "", nil, "", false
	}
	if open := strings.IndexByte(line, '('); open >= 0 {
		close, ok := findMatchingParen(line, open)
		if !ok {
			return "", nil, "", false
		}
		flagName := line[:open]
		paramsStr := line[open+1 : close]
		pairs := splitContextParams(paramsStr)
		rest := line[close+1:]
		eq := strings.Index(rest, "==")
		if eq < 0 {
			return "", nil, "", false
		}
		expected := strings.TrimSpace(rest[eq+2:])
		return flagName, pairs, expected, true
	}
	eq := strings.Index(line, "==")
	if eq < 0 {
		return "", nil, "", false
	}
	flagName := strings.TrimSpace(line[:eq])
	expected := strings.TrimSpace(line[eq+2:])
	return flagName, nil, expected, true
}

func findMatchingParen(s string, openPos int) (int, bool) {
	depth := 0
	inQuote := false
	bracketDepth := 0
	for i := openPos; i < len(s); i++ {
		ch := s[i]
		switch {
		case ch == '"':
			inQuote = !inQuote
		case ch == '[' && !inQuote:
			bracketDepth++
		case ch == ']' && !inQuote:
			bracketDepth--
		case ch == '(' && !inQuote && bracketDepth == 0:
			depth++
		case ch == ')' && !inQuote && bracketDepth == 0:
			depth--
			if depth == 0 {
				return i, true
			}
		}
	}
	return 0, false
}

func splitContextParams(s string) [][2]string {
	var pairs [][2]string
	bracketDepth := 0
	inQuote := false
	start := 0
	for i := 0; i < len(s); i++ {
		ch := s[i]
		switch {
		case ch == '"':
			inQuote = !inQuote
		case ch == '[' && !inQuote:
			bracketDepth++
		case ch == ']' && !inQuote:
			bracketDepth--
		case ch == ',' && !inQuote && bracketDepth == 0:
			if kv, ok := parseKvPair(s[start:i]); ok {
				pairs = append(pairs, kv)
			}
			start = i + 1
		}
	}
	if start < len(s) {
		if kv, ok := parseKvPair(s[start:]); ok {
			pairs = append(pairs, kv)
		}
	}
	return pairs
}

func parseKvPair(s string) ([2]string, bool) {
	s = strings.TrimSpace(s)
	eq := strings.IndexByte(s, '=')
	if eq < 0 {
		return [2]string{}, false
	}
	return [2]string{s[:eq], s[eq+1:]}, true
}

// ResultMatches compares an evaluation result with the expected string.
func ResultMatches(result FlagReturn, expected string) bool {
	switch result.Kind {
	case FROnOff:
		switch strings.ToUpper(expected) {
		case "TRUE":
			return result.Bool
		case "FALSE":
			return !result.Bool
		}
		return false
	case FRJson:
		var exp interface{}
		if err := json.Unmarshal([]byte(expected), &exp); err != nil {
			return false
		}
		return reflect.DeepEqual(result.Json, exp)
	case FRInt:
		n, err := strconv.ParseInt(expected, 10, 64)
		return err == nil && n == result.Int
	case FRStr:
		expectedStr := expected
		if len(expectedStr) >= 2 && strings.HasPrefix(expectedStr, "\"") && strings.HasSuffix(expectedStr, "\"") {
			expectedStr = expectedStr[1 : len(expectedStr)-1]
		}
		return result.Str == expectedStr
	}
	return false
}
