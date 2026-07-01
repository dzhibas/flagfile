package flagfile

import (
	"crypto/sha1"
	"encoding/hex"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// Context maps variable names to atom values.
type Context map[string]Atom

// Segments maps segment names to their boolean expression.
type Segments map[string]*AstNode

// ContextFrom builds a Context from string values (mirrors CLI test context building).
func ContextFrom(pairs map[string]string) Context {
	ctx := make(Context, len(pairs))
	for k, v := range pairs {
		ctx[k] = AtomFrom(v)
	}
	return ctx
}

// getVariableValue mirrors Rust `get_variable_value_from_context`.
func getVariableValue(node *AstNode, ctx Context) (Atom, bool) {
	if node == nil {
		return Atom{}, false
	}
	switch node.Type {
	case NVariable:
		if node.Atom.Kind == KVariable {
			v, ok := ctx[node.Atom.Str]
			return v, ok
		}
	case NConstant:
		if node.Atom.Kind == KVariable {
			v, ok := ctx[node.Atom.Str]
			return v, ok
		}
	case NFunction:
		switch node.Fn {
		case FnNow:
			return atomDateTime(time.Now().UTC()), true
		default:
			val, ok := getVariableValue(node.Child, ctx)
			if !ok {
				return Atom{}, false
			}
			switch node.Fn {
			case FnUpper:
				return atomString(strings.ToUpper(val.String())), true
			case FnLower:
				return atomString(strings.ToLower(val.String())), true
			}
		}
	case NCoalesce:
		for idx := range node.Args {
			arg := &node.Args[idx]
			switch arg.Type {
			case NVariable:
				if arg.Atom.Kind == KVariable {
					if v, ok := ctx[arg.Atom.Str]; ok {
						return v, true
					}
				}
			case NConstant:
				return arg.Atom, true
			}
		}
		return Atom{}, false
	}
	return Atom{}, false
}

// Eval evaluates an expression without segments.
func Eval(expr *AstNode, ctx Context, flagName string) bool {
	return evalImpl(expr, ctx, flagName, nil)
}

// EvalWithSegments evaluates an expression with segment resolution.
func EvalWithSegments(expr *AstNode, ctx Context, flagName string, segments Segments) bool {
	return evalImpl(expr, ctx, flagName, segments)
}

func evalImpl(expr *AstNode, ctx Context, flagName string, segments Segments) bool {
	if expr == nil {
		return false
	}
	switch expr.Type {
	case NConstant:
		if expr.Atom.Kind == KBoolean {
			return expr.Atom.Bool
		}
		if expr.Atom.Kind == KVariable {
			if v, ok := getVariableValue(expr, ctx); ok && v.Kind == KBoolean {
				return v.Bool
			}
		}
		return false

	case NCompare:
		cval, ok := getVariableValue(expr.Left, ctx)
		if !ok {
			return false
		}
		if expr.Right == nil || expr.Right.Type != NConstant {
			return false
		}
		val := expr.Right.Atom
		switch expr.CompOp {
		case OpEq:
			return atomEq(cval, val)
		case OpNotEq:
			return !atomEq(cval, val)
		default:
			ord, cmpOk := atomCmp(cval, val)
			if !cmpOk {
				return false
			}
			switch expr.CompOp {
			case OpMore:
				return ord > 0
			case OpMoreEq:
				return ord >= 0
			case OpLess:
				return ord < 0
			case OpLessEq:
				return ord <= 0
			}
		}
		return false

	case NArray:
		// Case 1: variable in (literal_list)
		if expr.Right != nil && expr.Right.Type == NList {
			search, ok := getVariableValue(expr.Left, ctx)
			if !ok {
				return false
			}
			switch expr.ArrayOp {
			case ArrayIn:
				for _, it := range expr.Right.List {
					if atomEq(search, it) {
						return true
					}
				}
				return false
			case ArrayNotIn:
				for _, it := range expr.Right.List {
					if atomEq(search, it) {
						return false
					}
				}
				return true
			}
			return false
		}
		// Case 2: "literal" in variable (variable resolves to List)
		var search Atom
		var searchOk bool
		if expr.Left != nil && expr.Left.Type == NConstant && expr.Left.Atom.Kind != KVariable {
			search = expr.Left.Atom
			searchOk = true
		} else {
			search, searchOk = getVariableValue(expr.Left, ctx)
		}
		listVal, listOk := getVariableValue(expr.Right, ctx)
		if !searchOk || !listOk || listVal.Kind != KList {
			return false
		}
		found := false
		for _, it := range listVal.List {
			if atomEq(search, it) {
				found = true
				break
			}
		}
		if expr.ArrayOp == ArrayIn {
			return found
		}
		return !found

	case NMatch:
		cval, ok := getVariableValue(expr.Left, ctx)
		if !ok {
			return false
		}
		haystack := cval.String()
		if expr.Right == nil || expr.Right.Type != NConstant {
			return false
		}
		rhs := expr.Right.Atom
		if rhs.Kind == KRegex {
			matched := false
			if re, err := regexp.Compile(rhs.Str); err == nil {
				matched = re.MatchString(haystack)
			}
			switch expr.MatchOp {
			case MatchContains:
				return matched
			case MatchNotContains:
				return !matched
			default:
				return false
			}
		}
		needle := rhs.String()
		switch expr.MatchOp {
		case MatchContains:
			return strings.Contains(haystack, needle)
		case MatchNotContains:
			return !strings.Contains(haystack, needle)
		case MatchStartsWith:
			return strings.HasPrefix(haystack, needle)
		case MatchNotStartsWith:
			return !strings.HasPrefix(haystack, needle)
		case MatchEndsWith:
			return strings.HasSuffix(haystack, needle)
		case MatchNotEndsWith:
			return !strings.HasSuffix(haystack, needle)
		}
		return false

	case NLogic:
		l := evalImpl(expr.Left, ctx, flagName, segments)
		r := evalImpl(expr.Right, ctx, flagName, segments)
		if expr.LogicOp == LogicAnd {
			return l && r
		}
		return l || r

	case NScope:
		res := evalImpl(expr.Child, ctx, flagName, segments)
		if expr.Negate {
			return !res
		}
		return res

	case NSegment:
		if segments != nil {
			if segExpr, ok := segments[expr.SegmentName]; ok {
				return evalImpl(segExpr, ctx, flagName, segments)
			}
		}
		return false

	case NNullCheck:
		_, ok := getVariableValue(expr.Child, ctx)
		if expr.IsNull {
			return !ok
		}
		return ok

	case NPercentage:
		bucketKey, ok := getVariableValue(expr.Field, ctx)
		if !ok {
			return false
		}
		keyStr := bucketKey.String()
		flag := flagName
		if flag == "" {
			flag = "unknown"
		}
		var input string
		if expr.HasSalt {
			input = flag + "." + expr.Salt + "." + keyStr
		} else {
			input = flag + "." + keyStr
		}
		sum := sha1.Sum([]byte(input))
		hexStr := hex.EncodeToString(sum[:])
		substr := hexStr[:15]
		value, _ := strconv.ParseUint(substr, 16, 64)
		bucket := value % 100000
		threshold := uint64(expr.Rate * 1000.0)
		return bucket < threshold
	}
	return false
}
