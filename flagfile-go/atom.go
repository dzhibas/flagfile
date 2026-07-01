package flagfile

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

// AtomKind mirrors the Rust `Atom` enum variants.
type AtomKind int

const (
	KString AtomKind = iota
	KNumber
	KFloat
	KBoolean
	KVariable
	KDate
	KDateTime
	KSemver
	KRegex
	KList
)

// Atom is a tagged union mirroring the Rust `Atom` enum.
type Atom struct {
	Kind     AtomKind
	Str      string    // String, Variable, Regex
	Num      int       // Number (i32 in Rust)
	Float    float64   // Float
	Bool     bool      // Boolean
	Date     time.Time // Date (stored at midnight UTC)
	DateTime time.Time // DateTime (UTC)
	Major    uint32    // Semver
	Minor    uint32
	Patch    uint32
	List     []Atom // List
}

// ── Constructors ─────────────────────────────────────────────

func atomString(s string) Atom      { return Atom{Kind: KString, Str: s} }
func atomVariable(s string) Atom    { return Atom{Kind: KVariable, Str: s} }
func atomRegex(s string) Atom       { return Atom{Kind: KRegex, Str: s} }
func atomNumber(n int) Atom         { return Atom{Kind: KNumber, Num: n} }
func atomFloat(f float64) Atom      { return Atom{Kind: KFloat, Float: f} }
func atomBool(b bool) Atom          { return Atom{Kind: KBoolean, Bool: b} }
func atomDate(t time.Time) Atom     { return Atom{Kind: KDate, Date: t} }
func atomDateTime(t time.Time) Atom { return Atom{Kind: KDateTime, DateTime: t} }
func atomSemver(a, b, c uint32) Atom {
	return Atom{Kind: KSemver, Major: a, Minor: b, Patch: c}
}
func atomList(items []Atom) Atom { return Atom{Kind: KList, List: items} }

// String mirrors the Rust `Display` impl for Atom.
func (a Atom) String() string {
	switch a.Kind {
	case KString, KVariable:
		return a.Str
	case KRegex:
		return "/" + a.Str + "/"
	case KNumber:
		return strconv.Itoa(a.Num)
	case KFloat:
		return strconv.FormatFloat(a.Float, 'g', -1, 64)
	case KBoolean:
		if a.Bool {
			return "true"
		}
		return "false"
	case KDate:
		return a.Date.Format("2006-01-02")
	case KDateTime:
		return a.DateTime.Format("2006-01-02T15:04:05")
	case KSemver:
		return fmt.Sprintf("%d.%d.%d", a.Major, a.Minor, a.Patch)
	case KList:
		parts := make([]string, len(a.List))
		for i, it := range a.List {
			parts[i] = it.String()
		}
		return "[" + strings.Join(parts, ", ") + "]"
	}
	return ""
}

// floatToSemver tries to interpret a float as semver components (5.4 -> 5,4,0).
func floatToSemver(f float64) (uint32, uint32, uint32, bool) {
	s := strconv.FormatFloat(f, 'g', -1, 64)
	if idx := strings.IndexByte(s, '.'); idx >= 0 {
		majS, minS := s[:idx], s[idx+1:]
		maj, err1 := strconv.ParseUint(majS, 10, 32)
		min, err2 := strconv.ParseUint(minS, 10, 32)
		if err1 != nil || err2 != nil {
			return 0, 0, 0, false
		}
		return uint32(maj), uint32(min), 0, true
	}
	maj, err := strconv.ParseUint(s, 10, 32)
	if err != nil {
		return 0, 0, 0, false
	}
	return uint32(maj), 0, 0, true
}

func cmpU32Triple(a1, b1, c1, a2, b2, c2 uint32) int {
	if a1 != a2 {
		if a1 < a2 {
			return -1
		}
		return 1
	}
	if b1 != b2 {
		if b1 < b2 {
			return -1
		}
		return 1
	}
	if c1 != c2 {
		if c1 < c2 {
			return -1
		}
		return 1
	}
	return 0
}

func cmpFloat(a, b float64) (int, bool) {
	if a != a || b != b { // NaN
		return 0, false
	}
	if a < b {
		return -1, true
	}
	if a > b {
		return 1, true
	}
	return 0, true
}

func cmpTime(a, b time.Time) (int, bool) {
	return a.Compare(b), true
}

// atomCmp mirrors the Rust `PartialOrd` impl. Returns (ordering, comparable).
func atomCmp(self, other Atom) (int, bool) {
	switch self.Kind {
	case KNumber:
		switch other.Kind {
		case KNumber:
			if self.Num < other.Num {
				return -1, true
			} else if self.Num > other.Num {
				return 1, true
			}
			return 0, true
		case KFloat:
			return cmpFloat(float64(self.Num), other.Float)
		case KSemver:
			if self.Num < 0 {
				return 0, false
			}
			maj := uint32(self.Num)
			return cmpU32Triple(maj, 0, 0, other.Major, other.Minor, other.Patch), true
		}
	case KFloat:
		switch other.Kind {
		case KFloat:
			return cmpFloat(self.Float, other.Float)
		case KNumber:
			return cmpFloat(self.Float, float64(other.Num))
		case KSemver:
			maj, min, patch, ok := floatToSemver(self.Float)
			if !ok {
				return 0, false
			}
			return cmpU32Triple(maj, min, patch, other.Major, other.Minor, other.Patch), true
		}
	case KDate:
		switch other.Kind {
		case KDate:
			return cmpTime(self.Date, other.Date)
		case KDateTime:
			return cmpTime(self.Date, other.DateTime)
		}
	case KDateTime:
		switch other.Kind {
		case KDateTime:
			return cmpTime(self.DateTime, other.DateTime)
		case KDate:
			return cmpTime(self.DateTime, other.Date)
		}
	case KSemver:
		switch other.Kind {
		case KSemver:
			return cmpU32Triple(self.Major, self.Minor, self.Patch, other.Major, other.Minor, other.Patch), true
		case KFloat:
			maj, min, patch, ok := floatToSemver(other.Float)
			if !ok {
				return 0, false
			}
			return cmpU32Triple(self.Major, self.Minor, self.Patch, maj, min, patch), true
		case KNumber:
			if other.Num < 0 {
				return 0, false
			}
			maj := uint32(other.Num)
			return cmpU32Triple(self.Major, self.Minor, self.Patch, maj, 0, 0), true
		}
	}
	return 0, false
}

// atomEq mirrors the Rust `PartialEq` impl for Atom.
func atomEq(a, b Atom) bool {
	// Symmetric String/Variable comparisons.
	if (a.Kind == KString || a.Kind == KVariable) && (b.Kind == KString || b.Kind == KVariable) {
		return a.Str == b.Str
	}
	switch a.Kind {
	case KNumber:
		switch b.Kind {
		case KNumber:
			return a.Num == b.Num
		case KSemver:
			return a.Num >= 0 && uint32(a.Num) == b.Major && b.Minor == 0 && b.Patch == 0
		}
	case KFloat:
		switch b.Kind {
		case KFloat:
			return a.Float == b.Float
		case KSemver:
			maj, min, patch, ok := floatToSemver(a.Float)
			return ok && maj == b.Major && min == b.Minor && patch == b.Patch
		}
	case KBoolean:
		if b.Kind == KBoolean {
			return a.Bool == b.Bool
		}
	case KDate:
		switch b.Kind {
		case KDate:
			return a.Date.Equal(b.Date)
		case KDateTime:
			return a.Date.Equal(b.DateTime)
		}
	case KDateTime:
		switch b.Kind {
		case KDateTime:
			return a.DateTime.Equal(b.DateTime)
		case KDate:
			return a.DateTime.Equal(b.Date)
		}
	case KSemver:
		switch b.Kind {
		case KSemver:
			return a.Major == b.Major && a.Minor == b.Minor && a.Patch == b.Patch
		case KFloat:
			maj, min, patch, ok := floatToSemver(b.Float)
			return ok && a.Major == maj && a.Minor == min && a.Patch == patch
		case KNumber:
			return b.Num >= 0 && a.Major == uint32(b.Num) && a.Minor == 0 && a.Patch == 0
		}
	case KRegex:
		if b.Kind == KRegex {
			return a.Str == b.Str
		}
	case KList:
		if b.Kind == KList {
			if len(a.List) != len(b.List) {
				return false
			}
			for i := range a.List {
				if !atomEq(a.List[i], b.List[i]) {
					return false
				}
			}
			return true
		}
	}
	return false
}

// AtomFrom mirrors the Rust `From<&str> for Atom`.
func AtomFrom(val string) Atom {
	trimmed := strings.TrimSpace(val)
	if strings.HasPrefix(trimmed, "[") && strings.HasSuffix(trimmed, "]") {
		inner := trimmed[1 : len(trimmed)-1]
		var items []Atom
		inQuote := false
		start := 0
		for i := 0; i < len(inner); i++ {
			ch := inner[i]
			switch ch {
			case '"':
				inQuote = !inQuote
			case ',':
				if !inQuote {
					items = append(items, atomFromSingle(inner[start:i]))
					start = i + 1
				}
			}
		}
		if start < len(inner) {
			item := strings.TrimSpace(inner[start:])
			if item != "" {
				items = append(items, atomFromSingle(inner[start:]))
			}
		}
		return atomList(items)
	}
	if _, out, ok := parseAtom(val); ok {
		return out
	}
	return atomString(val)
}

// atomFromSingle mirrors Rust `Atom::from_single` (strips surrounding double quotes).
func atomFromSingle(val string) Atom {
	val = strings.TrimSpace(val)
	unquoted := val
	if len(unquoted) >= 2 && strings.HasPrefix(unquoted, "\"") && strings.HasSuffix(unquoted, "\"") {
		unquoted = unquoted[1 : len(unquoted)-1]
	}
	if _, out, ok := parseAtom(unquoted); ok {
		return out
	}
	return atomString(unquoted)
}
