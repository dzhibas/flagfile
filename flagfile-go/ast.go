package flagfile

// ComparisonOp mirrors the Rust enum.
type ComparisonOp int

const (
	OpEq ComparisonOp = iota
	OpMore
	OpLess
	OpMoreEq
	OpLessEq
	OpNotEq
)

// MatchOp mirrors the Rust enum.
type MatchOp int

const (
	MatchContains MatchOp = iota
	MatchNotContains
	MatchStartsWith
	MatchNotStartsWith
	MatchEndsWith
	MatchNotEndsWith
)

// LogicOp mirrors the Rust enum.
type LogicOp int

const (
	LogicAnd LogicOp = iota
	LogicOr
)

// ArrayOp mirrors the Rust enum.
type ArrayOp int

const (
	ArrayIn ArrayOp = iota
	ArrayNotIn
)

// FnCall mirrors the Rust enum.
type FnCall int

const (
	FnUpper FnCall = iota
	FnLower
	FnNow
)

// NodeType mirrors the Rust `AstNode` enum discriminant.
type NodeType int

const (
	NVoid NodeType = iota
	NVariable
	NFunction
	NConstant
	NList
	NCompare
	NMatch
	NArray
	NLogic
	NScope
	NPercentage
	NCoalesce
	NSegment
	NNullCheck
)

// AstNode mirrors the Rust `AstNode` enum.
type AstNode struct {
	Type NodeType

	Atom Atom // Variable, Constant

	Fn    FnCall   // Function
	Child *AstNode // Function arg, Scope expr, NullCheck variable

	List []Atom // List

	Left    *AstNode // Compare/Match/Array/Logic
	Right   *AstNode
	CompOp  ComparisonOp
	MatchOp MatchOp
	ArrayOp ArrayOp
	LogicOp LogicOp

	Negate bool // Scope

	Rate    float64 // Percentage
	Field   *AstNode
	Salt    string
	HasSalt bool

	Args []AstNode // Coalesce

	SegmentName string // Segment

	IsNull bool // NullCheck
}

func nodePtr(n AstNode) *AstNode { return &n }
