package flagfile

import (
	"strconv"
	"strings"
	"time"
)

// ── Low-level helpers ────────────────────────────────────────

func isWs(b byte) bool {
	return b == ' ' || b == '\t' || b == '\n' || b == '\r' || b == '\x0c' || b == '\x0b'
}

func skipWs(i string) string {
	j := 0
	for j < len(i) && isWs(i[j]) {
		j++
	}
	return i[j:]
}

func isAlpha(b byte) bool {
	return (b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z')
}

func isDigit(b byte) bool { return b >= '0' && b <= '9' }

func isAlnum(b byte) bool { return isAlpha(b) || isDigit(b) }

// takeDigits consumes a run of one or more ASCII digits.
func takeDigits(i string) (string, string, bool) {
	j := 0
	for j < len(i) && isDigit(i[j]) {
		j++
	}
	if j == 0 {
		return "", i, false
	}
	return i[:j], i[j:], true
}

func hasPrefixFold(s, prefix string) bool {
	if len(s) < len(prefix) {
		return false
	}
	return strings.EqualFold(s[:len(prefix)], prefix)
}

// ── Atom parsers (parse.rs) ──────────────────────────────────

func parseNumber(i string) (string, Atom, bool) {
	orig := i
	if strings.HasPrefix(i, "-") {
		i = i[1:]
	}
	digits, rest, ok := takeDigits(i)
	if !ok {
		return orig, Atom{}, false
	}
	consumed := orig[:len(orig)-len(rest)]
	n, err := strconv.Atoi(consumed)
	if err != nil {
		// try just the digit part on overflow
		n, _ = strconv.Atoi(digits)
	}
	return rest, atomNumber(n), true
}

func parseFloat(i string) (string, Atom, bool) {
	orig := i
	if len(i) > 0 && (i[0] == '+' || i[0] == '-') {
		i = i[1:]
	}
	matched := false
	if _, rest, ok := takeDigits(i); ok {
		// digit1 then '.' then opt(digit1)
		if len(rest) > 0 && rest[0] == '.' {
			rest = rest[1:]
			if _, r2, ok2 := takeDigits(rest); ok2 {
				rest = r2
			}
			i = rest
			matched = true
		}
	}
	if !matched {
		// '.' then digit1
		if len(i) > 0 && i[0] == '.' {
			r := i[1:]
			if _, r2, ok2 := takeDigits(r); ok2 {
				i = r2
				matched = true
			}
		}
	}
	if !matched {
		return orig, Atom{}, false
	}
	// optional exponent
	if len(i) > 0 && (i[0] == 'e' || i[0] == 'E') {
		r := i[1:]
		if len(r) > 0 && (r[0] == '+' || r[0] == '-') {
			r = r[1:]
		}
		if _, r2, ok2 := takeDigits(r); ok2 {
			i = r2
		}
	}
	consumed := orig[:len(orig)-len(i)]
	f, err := strconv.ParseFloat(consumed, 64)
	if err != nil {
		return orig, Atom{}, false
	}
	return i, atomFloat(f), true
}

func parseBoolean(i string) (string, Atom, bool) {
	if hasPrefixFold(i, "true") {
		return i[4:], atomBool(true), true
	}
	if hasPrefixFold(i, "false") {
		return i[5:], atomBool(false), true
	}
	return i, Atom{}, false
}

// matchDatePattern matches digits '-' digits '-' digits, returns matched substring.
func matchDatePattern(i string) (string, string, bool) {
	orig := i
	if _, r, ok := takeDigits(i); ok {
		i = r
	} else {
		return "", orig, false
	}
	if len(i) == 0 || i[0] != '-' {
		return "", orig, false
	}
	i = i[1:]
	if _, r, ok := takeDigits(i); ok {
		i = r
	} else {
		return "", orig, false
	}
	if len(i) == 0 || i[0] != '-' {
		return "", orig, false
	}
	i = i[1:]
	if _, r, ok := takeDigits(i); ok {
		i = r
	} else {
		return "", orig, false
	}
	return orig[:len(orig)-len(i)], i, true
}

func parseDate(i string) (string, Atom, bool) {
	matched, rest, ok := matchDatePattern(i)
	if !ok {
		return i, Atom{}, false
	}
	t, err := time.Parse("2006-01-02", matched)
	if err != nil {
		return i, Atom{}, false
	}
	return rest, atomDate(t.UTC()), true
}

func parseDateTime(i string) (string, Atom, bool) {
	orig := i
	dateStr, rest, ok := matchDatePattern(i)
	if !ok {
		return orig, Atom{}, false
	}
	if len(rest) == 0 || rest[0] != 'T' {
		return orig, Atom{}, false
	}
	i = rest[1:]
	// digits ':' digits ':' digits
	part := func() bool {
		_, r, ok := takeDigits(i)
		if !ok {
			return false
		}
		i = r
		return true
	}
	if !part() {
		return orig, Atom{}, false
	}
	if len(i) == 0 || i[0] != ':' {
		return orig, Atom{}, false
	}
	i = i[1:]
	if !part() {
		return orig, Atom{}, false
	}
	if len(i) == 0 || i[0] != ':' {
		return orig, Atom{}, false
	}
	i = i[1:]
	if !part() {
		return orig, Atom{}, false
	}
	// optional Z
	full := orig[:len(orig)-len(i)]
	if len(i) > 0 && i[0] == 'Z' {
		i = i[1:]
	}
	clean := strings.TrimSuffix(full, "Z")
	_ = dateStr
	t, err := time.Parse("2006-01-02T15:04:05", clean)
	if err != nil {
		return orig, Atom{}, false
	}
	return i, atomDateTime(t.UTC()), true
}

func parseSemver(i string) (string, Atom, bool) {
	orig := i
	maj, r, ok := takeDigits(i)
	if !ok {
		return orig, Atom{}, false
	}
	i = r
	if len(i) == 0 || i[0] != '.' {
		return orig, Atom{}, false
	}
	i = i[1:]
	min, r, ok := takeDigits(i)
	if !ok {
		return orig, Atom{}, false
	}
	i = r
	if len(i) == 0 || i[0] != '.' {
		return orig, Atom{}, false
	}
	i = i[1:]
	patch, r, ok := takeDigits(i)
	if !ok {
		return orig, Atom{}, false
	}
	i = r
	majN, _ := strconv.ParseUint(maj, 10, 32)
	minN, _ := strconv.ParseUint(min, 10, 32)
	patchN, _ := strconv.ParseUint(patch, 10, 32)
	return i, atomSemver(uint32(majN), uint32(minN), uint32(patchN)), true
}

func parseStringAtom(i string) (string, Atom, bool) {
	if len(i) > 0 && i[0] == '"' {
		idx := strings.IndexByte(i[1:], '"')
		if idx < 0 {
			return i, Atom{}, false
		}
		return i[1+idx+1:], atomString(i[1 : 1+idx]), true
	}
	if len(i) > 0 && i[0] == '\'' {
		idx := strings.IndexByte(i[1:], '\'')
		if idx < 0 {
			return i, Atom{}, false
		}
		return i[1+idx+1:], atomString(i[1 : 1+idx]), true
	}
	return i, Atom{}, false
}

func parseVariable(i string) (string, Atom, bool) {
	if len(i) == 0 || !(isAlpha(i[0]) || i[0] == '_') {
		return i, Atom{}, false
	}
	j := 1
	for j < len(i) && (isAlnum(i[j]) || i[j] == '_') {
		j++
	}
	return i[j:], atomVariable(i[:j]), true
}

func parseAtom(i string) (string, Atom, bool) {
	for _, p := range []func(string) (string, Atom, bool){
		parseDateTime,
		parseDate,
		parseStringAtom,
		parseBoolean,
		parseSemver,
		parseFloat,
		parseNumber,
		parseVariable,
	} {
		if rest, a, ok := p(i); ok {
			return rest, a, true
		}
	}
	return i, Atom{}, false
}

// ── Operator parsers ─────────────────────────────────────────

func parseComparisonOp(i string) (string, ComparisonOp, bool) {
	switch {
	case strings.HasPrefix(i, "!="), strings.HasPrefix(i, "<>"):
		return i[2:], OpNotEq, true
	case strings.HasPrefix(i, "=="):
		return i[2:], OpEq, true
	case strings.HasPrefix(i, "="):
		return i[1:], OpEq, true
	case strings.HasPrefix(i, "<="):
		return i[2:], OpLessEq, true
	case strings.HasPrefix(i, "<"):
		return i[1:], OpLess, true
	case strings.HasPrefix(i, ">="):
		return i[2:], OpMoreEq, true
	case strings.HasPrefix(i, ">"):
		return i[1:], OpMore, true
	}
	return i, 0, false
}

func parseLogicOp(i string) (string, LogicOp, bool) {
	if strings.HasPrefix(i, "&&") {
		return i[2:], LogicAnd, true
	}
	if hasPrefixFold(i, "and") {
		return i[3:], LogicAnd, true
	}
	if strings.HasPrefix(i, "||") {
		return i[2:], LogicOr, true
	}
	if hasPrefixFold(i, "or") {
		return i[2:], LogicOr, true
	}
	return i, 0, false
}

func parseArrayOp(i string) (string, ArrayOp, bool) {
	if hasPrefixFold(i, "not in") {
		return i[6:], ArrayNotIn, true
	}
	if hasPrefixFold(i, "in") {
		return i[2:], ArrayIn, true
	}
	return i, 0, false
}

func parseFunctionNames(i string) (string, FnCall, bool) {
	if hasPrefixFold(i, "upper") {
		return i[5:], FnUpper, true
	}
	if hasPrefixFold(i, "lower") {
		return i[5:], FnLower, true
	}
	return i, 0, false
}

func parseMatchOp(i string) (string, MatchOp, bool) {
	switch {
	case strings.HasPrefix(i, "!^~"):
		return i[3:], MatchNotStartsWith, true
	case strings.HasPrefix(i, "!~$"):
		return i[3:], MatchNotEndsWith, true
	case strings.HasPrefix(i, "!~"):
		return i[2:], MatchNotContains, true
	case strings.HasPrefix(i, "^~"):
		return i[2:], MatchStartsWith, true
	case strings.HasPrefix(i, "~$"):
		return i[2:], MatchEndsWith, true
	case strings.HasPrefix(i, "~"):
		return i[1:], MatchContains, true
	}
	return i, 0, false
}

// ── Node parsers ─────────────────────────────────────────────

func parseList(i string) (string, AstNode, bool) {
	if len(i) == 0 || i[0] != '(' {
		return i, AstNode{}, false
	}
	i = i[1:]
	var items []Atom
	// separated_list0(",", ws(parseAtom))
	rest := skipWs(i)
	if r2, a, ok := parseAtom(rest); ok {
		items = append(items, a)
		i = skipWs(r2)
		for len(i) > 0 && i[0] == ',' {
			i = i[1:]
			i = skipWs(i)
			r3, a2, ok2 := parseAtom(i)
			if !ok2 {
				return i, AstNode{}, false
			}
			items = append(items, a2)
			i = skipWs(r3)
		}
	}
	if len(i) == 0 || i[0] != ')' {
		return i, AstNode{}, false
	}
	return i[1:], AstNode{Type: NList, List: items}, true
}

func parseVariableNode(i string) (string, AstNode, bool) {
	rest, a, ok := parseVariable(i)
	if !ok {
		return i, AstNode{}, false
	}
	return rest, AstNode{Type: NVariable, Atom: a}, true
}

func parseConstant(i string) (string, AstNode, bool) {
	rest, a, ok := parseAtom(i)
	if !ok {
		return i, AstNode{}, false
	}
	return rest, AstNode{Type: NConstant, Atom: a}, true
}

func parseVariableNodeModifier(i string) (string, AstNode, bool) {
	orig := i
	r := skipWs(i)
	r, fn, ok := parseFunctionNames(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	if len(r) == 0 || r[0] != '(' {
		return orig, AstNode{}, false
	}
	r = r[1:]
	r = skipWs(r)
	r, inner, ok := parseVariableNode(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	if len(r) == 0 || r[0] != ')' {
		return orig, AstNode{}, false
	}
	r = r[1:]
	return r, AstNode{Type: NFunction, Fn: fn, Child: nodePtr(inner)}, true
}

func parseNullaryFunction(i string) (string, AstNode, bool) {
	if hasPrefixFold(i, "now()") {
		return i[5:], AstNode{Type: NFunction, Fn: FnNow, Child: nodePtr(AstNode{Type: NVoid})}, true
	}
	return i, AstNode{}, false
}

func parseCoalesceArg(i string) (string, AstNode, bool) {
	if r, n, ok := parseVariableNode(i); ok {
		return r, n, true
	}
	return parseConstant(i)
}

func parseCoalesce(i string) (string, AstNode, bool) {
	orig := i
	if !hasPrefixFold(i, "coalesce") {
		return orig, AstNode{}, false
	}
	i = i[len("coalesce"):]
	i = skipWs(i)
	if len(i) == 0 || i[0] != '(' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	var args []AstNode
	// separated_list0(ws(','), ws(parseCoalesceArg))
	r := skipWs(i)
	if r2, n, ok := parseCoalesceArg(r); ok {
		args = append(args, n)
		i = skipWs(r2)
		for len(i) > 0 && i[0] == ',' {
			i = i[1:]
			i = skipWs(i)
			r3, n2, ok2 := parseCoalesceArg(i)
			if !ok2 {
				return orig, AstNode{}, false
			}
			args = append(args, n2)
			i = skipWs(r3)
		}
	} else {
		i = r
	}
	if len(args) < 2 {
		return orig, AstNode{}, false
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != ')' {
		return orig, AstNode{}, false
	}
	return i[1:], AstNode{Type: NCoalesce, Args: args}, true
}

func parseVariableNodeOrModified(i string) (string, AstNode, bool) {
	for _, p := range []func(string) (string, AstNode, bool){
		parseCoalesce,
		parseNullaryFunction,
		parseVariableNodeModifier,
		parseVariableNode,
	} {
		if r, n, ok := p(i); ok {
			return r, n, true
		}
	}
	return i, AstNode{}, false
}

func parseSegmentName(i string) (string, string, bool) {
	if len(i) == 0 || !(isAlpha(i[0]) || i[0] == '_') {
		return i, "", false
	}
	j := 1
	for j < len(i) && (isAlnum(i[j]) || i[j] == '_' || i[j] == '-') {
		j++
	}
	return i[j:], i[:j], true
}

func parseSegmentCall(i string) (string, AstNode, bool) {
	orig := i
	if !hasPrefixFold(i, "segment") {
		return orig, AstNode{}, false
	}
	i = i[len("segment"):]
	if len(i) == 0 || i[0] != '(' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	i = skipWs(i)
	i, name, ok := parseSegmentName(i)
	if !ok {
		return orig, AstNode{}, false
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != ')' {
		return orig, AstNode{}, false
	}
	return i[1:], AstNode{Type: NSegment, SegmentName: name}, true
}

func parseArrayExpr(i string) (string, AstNode, bool) {
	orig := i
	r, left, ok := parseVariableNodeOrModified(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, op, ok := parseArrayOp(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, list, ok := parseList(r)
	if !ok {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NArray, Left: nodePtr(left), ArrayOp: op, Right: nodePtr(list)}, true
}

func parseCompareExpr(i string) (string, AstNode, bool) {
	orig := i
	r, left, ok := parseVariableNodeOrModified(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, op, ok := parseComparisonOp(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, right, ok := parseConstant(r)
	if !ok {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NCompare, Left: nodePtr(left), CompOp: op, Right: nodePtr(right)}, true
}

func parseRegexLiteral(i string) (string, Atom, bool) {
	if len(i) == 0 || i[0] != '/' {
		return i, Atom{}, false
	}
	idx := strings.IndexByte(i[1:], '/')
	if idx < 0 {
		return i, Atom{}, false
	}
	pattern := i[1 : 1+idx]
	return i[1+idx+1:], atomRegex(pattern), true
}

func parseMatchRhs(i string) (string, AstNode, bool) {
	if r, a, ok := parseRegexLiteral(i); ok {
		return r, AstNode{Type: NConstant, Atom: a}, true
	}
	return parseConstant(i)
}

func parseMatchExpr(i string) (string, AstNode, bool) {
	orig := i
	r, left, ok := parseVariableNodeOrModified(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, op, ok := parseMatchOp(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, right, ok := parseMatchRhs(r)
	if !ok {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NMatch, Left: nodePtr(left), MatchOp: op, Right: nodePtr(right)}, true
}

func parseReverseArrayExpr(i string) (string, AstNode, bool) {
	orig := i
	r, left, ok := parseConstant(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, op, ok := parseArrayOp(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, right, ok := parseVariableNode(r)
	if !ok {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NArray, Left: nodePtr(left), ArrayOp: op, Right: nodePtr(right)}, true
}

func parseNullCheck(i string) (string, AstNode, bool) {
	orig := i
	r, variable, ok := parseVariableNodeOrModified(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	if !hasPrefixFold(r, "is") {
		return orig, AstNode{}, false
	}
	r = r[2:]
	r = skipWs(r)
	negated := false
	if hasPrefixFold(r, "not") {
		negated = true
		r = r[3:]
		r = skipWs(r)
	}
	if !hasPrefixFold(r, "null") {
		return orig, AstNode{}, false
	}
	r = r[4:]
	// 'null' must not be followed by word characters
	if len(r) > 0 && (isAlnum(r[0]) || r[0] == '_') {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NNullCheck, Child: nodePtr(variable), IsNull: !negated}, true
}

func parseCompareOrArrayExpr(i string) (string, AstNode, bool) {
	for _, p := range []func(string) (string, AstNode, bool){
		parseNullCheck,
		parseArrayExpr,
		parseReverseArrayExpr,
		parseMatchExpr,
		parseCompareExpr,
	} {
		if r, n, ok := p(i); ok {
			return r, n, true
		}
	}
	return i, AstNode{}, false
}

func parseCompareOrArrayOrParen(i string) (string, AstNode, bool) {
	if r, n, ok := parseCompareOrArrayExpr(i); ok {
		return r, n, true
	}
	return parseParenthesizedExpr(i)
}

func parseLogicExpr(i string) (string, AstNode, bool) {
	orig := i
	r, left, ok := parseCompareOrArrayOrParen(i)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, op, ok := parseLogicOp(r)
	if !ok {
		return orig, AstNode{}, false
	}
	r = skipWs(r)
	r, right, ok := parseCompareOrArrayOrParen(r)
	if !ok {
		return orig, AstNode{}, false
	}
	return r, AstNode{Type: NLogic, Left: nodePtr(left), LogicOp: op, Right: nodePtr(right)}, true
}

func parsePercentageSalt(i string) (string, string, bool) {
	orig := i
	i = skipWs(i)
	if len(i) == 0 || i[0] != ',' {
		return orig, "", false
	}
	i = i[1:]
	i = skipWs(i)
	if len(i) == 0 || !(isAlpha(i[0]) || i[0] == '_') {
		return orig, "", false
	}
	j := 1
	for j < len(i) && (isAlnum(i[j]) || i[j] == '_' || i[j] == '-') {
		j++
	}
	return i[j:], i[:j], true
}

func parsePercentage(i string) (string, AstNode, bool) {
	orig := i
	if !hasPrefixFold(i, "percentage") {
		return orig, AstNode{}, false
	}
	i = i[len("percentage"):]
	i = skipWs(i)
	if len(i) == 0 || i[0] != '(' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	// rate: opt(sign) then float-or-int form
	rateStart := i
	if len(i) > 0 && (i[0] == '+' || i[0] == '-') {
		i = i[1:]
	}
	// digits ('.' optdigits)? | '.' digits | digits
	matched := false
	if _, r, ok := takeDigits(i); ok {
		i = r
		if len(i) > 0 && i[0] == '.' {
			i = i[1:]
			if _, r2, ok2 := takeDigits(i); ok2 {
				i = r2
			}
		}
		matched = true
	} else if len(i) > 0 && i[0] == '.' {
		r := i[1:]
		if _, r2, ok2 := takeDigits(r); ok2 {
			i = r2
			matched = true
		}
	}
	if !matched {
		return orig, AstNode{}, false
	}
	rateStr := rateStart[:len(rateStart)-len(i)]
	if len(i) == 0 || i[0] != '%' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	rate, err := strconv.ParseFloat(rateStr, 64)
	if err != nil {
		return orig, AstNode{}, false
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != ',' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	i = skipWs(i)
	i, field, ok := parseVariableNode(i)
	if !ok {
		return orig, AstNode{}, false
	}
	i = skipWs(i)
	salt := ""
	hasSalt := false
	if r, s, ok := parsePercentageSalt(i); ok {
		salt = s
		hasSalt = true
		i = r
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != ')' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	return i, AstNode{Type: NPercentage, Rate: rate, Field: nodePtr(field), Salt: salt, HasSalt: hasSalt}, true
}

func parseParenthesizedExpr(i string) (string, AstNode, bool) {
	orig := i
	negate := false
	if hasPrefixFold(i, "not") {
		negate = true
		i = i[3:]
	} else if strings.HasPrefix(i, "!") {
		negate = true
		i = i[1:]
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != '(' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	i, expr, ok := parseExpr(i)
	if !ok {
		return orig, AstNode{}, false
	}
	i = skipWs(i)
	if len(i) == 0 || i[0] != ')' {
		return orig, AstNode{}, false
	}
	i = i[1:]
	return i, AstNode{Type: NScope, Child: nodePtr(expr), Negate: negate}, true
}

func parseExpr(input string) (string, AstNode, bool) {
	var head AstNode
	var ok bool
	i := input
	for _, p := range []func(string) (string, AstNode, bool){
		parseParenthesizedExpr,
		parsePercentage,
		parseSegmentCall,
		parseLogicExpr,
		parseCompareOrArrayExpr,
		parseConstant,
	} {
		if r, n, o := p(i); o {
			i = r
			head = n
			ok = true
			break
		}
	}
	if !ok {
		return input, AstNode{}, false
	}
	// tail: many0(pair(ws(logicOp), alt(percentage, segment, compareOrArray, paren)))
	for {
		r := skipWs(i)
		r, op, opOk := parseLogicOp(r)
		if !opOk {
			break
		}
		r = skipWs(r)
		var next AstNode
		nextOk := false
		for _, p := range []func(string) (string, AstNode, bool){
			parsePercentage,
			parseSegmentCall,
			parseCompareOrArrayExpr,
			parseParenthesizedExpr,
		} {
			if r2, n, o := p(r); o {
				r = r2
				next = n
				nextOk = true
				break
			}
		}
		if !nextOk {
			break
		}
		head = AstNode{Type: NLogic, Left: nodePtr(head), LogicOp: op, Right: nodePtr(next)}
		i = r
	}
	return i, head, true
}

// Parse mirrors the top-level Rust `parse`.
func Parse(i string) (string, AstNode, bool) {
	r := skipWs(i)
	if r2, n, ok := parseExpr(r); ok {
		return skipWs(r2), n, true
	}
	r = skipWs(i)
	if r2, n, ok := parseParenthesizedExpr(r); ok {
		return skipWs(r2), n, true
	}
	return i, AstNode{}, false
}
