package flagfile

import "testing"

// helper: parse an expression fully, requiring empty remainder.
func mustParse(t *testing.T, s string) AstNode {
	t.Helper()
	rest, node, ok := Parse(s)
	if !ok {
		t.Fatalf("parse failed for %q", s)
	}
	if rest != "" {
		t.Fatalf("parse of %q left remainder %q", s, rest)
	}
	return node
}

func TestParseAtoms(t *testing.T) {
	if _, a, ok := parseAtom("5.3.42"); !ok || a.Kind != KSemver || a.Major != 5 || a.Minor != 3 || a.Patch != 42 {
		t.Fatalf("semver parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("4.32"); !ok || a.Kind != KFloat || a.Float != 4.32 {
		t.Fatalf("float parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("3"); !ok || a.Kind != KNumber || a.Num != 3 {
		t.Fatalf("number parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("-10"); !ok || a.Kind != KNumber || a.Num != -10 {
		t.Fatalf("neg number parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("TRUE"); !ok || a.Kind != KBoolean || !a.Bool {
		t.Fatalf("bool parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("_demo_demo"); !ok || a.Kind != KVariable || a.Str != "_demo_demo" {
		t.Fatalf("variable parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("2025-06-15T09:00:00Z"); !ok || a.Kind != KDateTime {
		t.Fatalf("datetime parse: %+v ok=%v", a, ok)
	}
	if _, a, ok := parseAtom("2025-06-15"); !ok || a.Kind != KDate {
		t.Fatalf("date parse: %+v ok=%v", a, ok)
	}
}

func TestParseExpressions(t *testing.T) {
	cases := []string{
		"a=b and c=d and (dd not in (1,2,3) or z == \"demo car\")",
		"not (a=b and c=d)",
		"UPPER(_demo) == 'DEMO DEMO'",
		"now() > 2025-06-15T09:00:00Z and now() < 2025-06-15T18:00:00Z",
		"name ~ /.*ola.*/",
		"upper(name) ~ /.*OLA.*/",
		"path ^~ \"/admin\"",
		"email ~$ \"@company.com\"",
		"name !^~ \"test\"",
		"name !~$ \".tmp\"",
		"coalesce(a, b, \"default\") == \"test\"",
		"\"admin\" in roles",
		"\"admin\" not in roles",
		"userId is null",
		"userId is not null and plan == premium",
		"segment(beta_users)",
		"segment(premium-users)",
		"percentage(50%, userId)",
		"percentage(25%, orgId, experiment_1)",
		"a = b and c=d or e = f",
	}
	for _, c := range cases {
		mustParse(t, c)
	}
}

func TestEvalBasics(t *testing.T) {
	check := func(expr string, ctx Context, want bool) {
		t.Helper()
		node := mustParse(t, expr)
		if got := Eval(&node, ctx, ""); got != want {
			t.Fatalf("eval %q = %v, want %v", expr, got, want)
		}
	}
	check("x=1 and y=2", Context{"x": atomNumber(1), "y": atomNumber(2)}, true)
	check("countryCode==LT && city='Palanga'", Context{"countryCode": atomString("LT"), "city": atomString("Palanga")}, true)
	check("lower(countryCode)==lt && upper(city)='PALANGA'", Context{"countryCode": atomString("LT"), "city": atomString("Palanga")}, true)
	check("!(country=LT)", Context{"country": atomString("LT")}, false)
	check("a < 4", Context{"a": atomNumber(3)}, true)
	check("a>=3", Context{"a": atomNumber(3)}, true)
	check("a!=4", Context{"a": atomNumber(3)}, true)
	check("version > 5.3.42", Context{"version": atomSemver(6, 0, 0)}, true)
	check("version > 5.3.42", Context{"version": atomSemver(5, 3, 42)}, false)
	check("version < 4.32.0", Context{"version": atomSemver(4, 31, 9)}, true)
	check("version > 5.3.42", Context{"version": atomFloat(5.4)}, true)
	check("version > 5.3.42", Context{"version": atomFloat(5.3)}, false)
	check("y in ('one','two','tree')", Context{"y": atomString("tree")}, true)
	check("y not in ('one','two','tree')", Context{"y": atomString("four")}, true)
	check("name ~ Nik", Context{"name": atomString("Nikolajus")}, true)
	check("name !~ Nik", Context{"name": atomString("John")}, true)
	check("name ~ /.*ola.*/", Context{"name": atomString("Nikolajus")}, true)
	check("path ^~ \"/admin\"", Context{"path": atomString("/admin/settings")}, true)
	check("email ~$ \"@company.com\"", Context{"email": atomString("user@company.com")}, true)
	check("userId is null", Context{}, true)
	check("userId is not null and plan == premium", Context{"userId": atomString("a"), "plan": atomString("premium")}, true)
	check("coalesce(countryCode, region, \"unknown\") == \"unknown\"", Context{}, true)
	check("created > 2024-02-02 and created <= 2024-02-13", Context{"created": AtomFrom("2024-02-12")}, true)
	// reverse in with array context
	roles := atomList([]Atom{atomString("viewer"), atomString("editor"), atomString("admin")})
	check("\"admin\" in roles", Context{"roles": roles}, true)
	check("\"superadmin\" in roles", Context{"roles": roles}, false)
}

func TestEvalSegments(t *testing.T) {
	seg := mustParse(t, "plan == premium")
	segments := Segments{"premium_users": &seg}
	node := mustParse(t, "segment(premium_users)")
	if !EvalWithSegments(&node, Context{"plan": atomString("premium")}, "", segments) {
		t.Fatal("segment should be true")
	}
	if EvalWithSegments(&node, Context{"plan": atomString("free")}, "", segments) {
		t.Fatal("segment should be false")
	}
	missing := mustParse(t, "segment(nonexistent)")
	if EvalWithSegments(&missing, Context{}, "", segments) {
		t.Fatal("missing segment should be false")
	}
}

// Cross-language percentage vectors (must match Rust/TS).
func TestPercentageVectors(t *testing.T) {
	e50 := mustParse(t, "percentage(50%, userId)")
	if !Eval(&e50, Context{"userId": atomString("user-123")}, "FF-test-rollout") {
		t.Fatal("vector1 want true")
	}
	if Eval(&e50, Context{"userId": atomString("user-456")}, "FF-test-rollout") {
		t.Fatal("vector2 want false")
	}
	if !Eval(&e50, Context{"userId": atomString("user-789")}, "FF-new-checkout") {
		t.Fatal("vector3 want true")
	}
	eSalt := mustParse(t, "percentage(50%, userId, exp1)")
	if Eval(&eSalt, Context{"userId": atomString("alice")}, "FF-test-rollout") {
		t.Fatal("vector4 (salt) want false")
	}
	eZero := mustParse(t, "percentage(0%, userId)")
	if Eval(&eZero, Context{"userId": atomString("user-123")}, "FF-test-rollout") {
		t.Fatal("0% want false")
	}
	eFull := mustParse(t, "percentage(100%, userId)")
	if !Eval(&eFull, Context{"userId": atomString("user-123")}, "FF-test-rollout") {
		t.Fatal("100% want true")
	}
	if Eval(&e50, Context{"userId": atomString("alice")}, "FF-test") {
		t.Fatal("vector7 want false")
	}
}
