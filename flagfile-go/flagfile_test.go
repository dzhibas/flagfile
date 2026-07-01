package flagfile

import (
	"os"
	"strings"
	"testing"
)

func loadFlagfile(t *testing.T, path string) (string, ParsedFlagfile) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	rest, parsed := ParseFlagfileWithSegments(string(data))
	if strings.TrimSpace(rest) != "" {
		t.Fatalf("parse of %s left remainder near: %q", path, firstLine(rest))
	}
	return string(data), parsed
}

func firstLine(s string) string {
	s = strings.TrimSpace(s)
	if i := strings.IndexByte(s, '\n'); i >= 0 {
		return s[:i]
	}
	return s
}

func buildFlagMap(parsed ParsedFlagfile) map[string]FlagDefinition {
	flags := make(map[string]FlagDefinition, len(parsed.Flags))
	for _, e := range parsed.Flags {
		flags[e.Name] = e.Def
	}
	return flags
}

// runAssertion evaluates a single `FF-name(...) == expected` line.
func runAssertion(t *testing.T, line string, flags map[string]FlagDefinition, segments Segments) {
	t.Helper()
	name, pairs, expected, ok := ParseTestLine(line)
	if !ok {
		t.Fatalf("invalid test line: %q", line)
	}
	ctx := Context{}
	for _, kv := range pairs {
		ctx[kv[0]] = AtomFrom(kv[1])
	}
	if _, exists := flags[name]; !exists {
		t.Fatalf("flag not found: %s (line %q)", name, line)
	}
	res, matched := EvaluateFlag(name, ctx, flags, segments, "", false)
	if !matched {
		t.Fatalf("no rule matched: %q", line)
	}
	if !ResultMatches(res, expected) {
		t.Fatalf("assertion failed: %q (got %+v)", line, res)
	}
}

func TestFlagfileExampleParses(t *testing.T) {
	_, parsed := loadFlagfile(t, "../Flagfile.example")
	if len(parsed.Flags) == 0 {
		t.Fatal("no flags parsed")
	}
	if len(parsed.Segments) == 0 {
		t.Fatal("no segments parsed")
	}
}

func TestFlagfileTestsFile(t *testing.T) {
	_, parsed := loadFlagfile(t, "../Flagfile.example")
	flags := buildFlagMap(parsed)

	data, err := os.ReadFile("../Flagfile.tests")
	if err != nil {
		t.Fatalf("read Flagfile.tests: %v", err)
	}
	count := 0
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "//") {
			continue
		}
		runAssertion(t, line, flags, parsed.Segments)
		count++
	}
	if count == 0 {
		t.Fatal("no test lines executed")
	}
	t.Logf("Flagfile.tests: %d assertions passed", count)
}

func TestFlagfileInlineAndMetadataTests(t *testing.T) {
	content, parsed := loadFlagfile(t, "../Flagfile.example")
	flags := buildFlagMap(parsed)

	// Inline @test annotations from comments.
	inline := ExtractTestAnnotations(content)
	for _, a := range inline {
		runAssertion(t, a.Assertion, flags, parsed.Segments)
	}

	// Standalone @test annotations parsed as flag metadata.
	metaCount := 0
	for _, e := range parsed.Flags {
		for _, assertion := range e.Def.Metadata.Tests {
			runAssertion(t, assertion, flags, parsed.Segments)
			metaCount++
		}
	}

	if len(inline) == 0 {
		t.Fatal("expected inline @test annotations")
	}
	if metaCount == 0 {
		t.Fatal("expected metadata @test annotations")
	}
	t.Logf("inline @test: %d passed, metadata @test: %d passed", len(inline), metaCount)
}
