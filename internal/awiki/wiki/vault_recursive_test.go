package wiki

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeFileAt(t *testing.T, root, rel, content string) {
	t.Helper()
	full := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatalf("mkdir for %s: %v", rel, err)
	}
	writeFile(t, full, content)
}

func loadRecursive(t *testing.T, dir string) *Vault {
	t.Helper()
	vault, err := LoadWithOptions(dir, Options{Recursive: true})
	if err != nil {
		t.Fatalf("LoadWithOptions(recursive) error = %v", err)
	}
	return vault
}

func resolvedTargets(doc *Document) []string {
	var out []string
	for _, link := range doc.Links {
		if link.Resolved != "" {
			out = append(out, link.Resolved)
		}
	}
	return out
}

func TestRecursiveDiscoversNestedDocumentsByPath(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "goals/login.md", "---\ntype: goal\n---\n# Login\n")
	writeFileAt(t, dir, "features/login.md", "# Feature\n\n[covers](../goals/login.md)\n")
	writeFileAt(t, dir, "top.md", "# Top\n")

	vault := loadRecursive(t, dir)
	if len(vault.Documents) != 3 {
		t.Fatalf("len(Documents) = %d, want 3", len(vault.Documents))
	}

	doc, err := vault.ResolveDocument("features/login")
	if err != nil {
		t.Fatalf("ResolveDocument(path) error = %v", err)
	}
	if doc.RelPath != "features/login" || doc.Name != "features/login" {
		t.Fatalf("identity = RelPath %q Name %q, want features/login", doc.RelPath, doc.Name)
	}
	if got := resolvedTargets(doc); len(got) != 1 || got[0] != "goals/login" {
		t.Fatalf("resolved = %v, want [goals/login]", got)
	}
}

func TestRecursiveAllowsDuplicateBasenames(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "a/Note.md", "# A Note\n")
	writeFileAt(t, dir, "b/Note.md", "# B Note\n")

	vault := loadRecursive(t, dir)
	if len(vault.Documents) != 2 {
		t.Fatalf("len(Documents) = %d, want 2 (duplicate basenames must coexist)", len(vault.Documents))
	}
	a, err := vault.ResolveDocument("a/Note")
	if err != nil {
		t.Fatalf("ResolveDocument(a/Note) error = %v", err)
	}
	b, err := vault.ResolveDocument("b/Note.md")
	if err != nil {
		t.Fatalf("ResolveDocument(b/Note.md) error = %v", err)
	}
	if a == b || a.RelPath != "a/Note" || b.RelPath != "b/Note" {
		t.Fatalf("path addressing failed: a=%q b=%q", a.RelPath, b.RelPath)
	}
}

func TestRecursiveObsidianResolutionPrecedence(t *testing.T) {
	dir := t.TempDir()
	// Two "shared" basenames: root/shortest path must win for a bare link.
	writeFileAt(t, dir, "shared.md", "# Root Shared\n")
	writeFileAt(t, dir, "deep/nest/shared.md", "# Deep Shared\n")
	writeFileAt(t, dir, "goals/login.md", "# Goal\n")
	writeFileAt(t, dir, "features/note.md",
		"# Note\n\nbare [[shared]]\npath [[goals/login]]\nrel [up](../goals/login.md)\n")

	vault := loadRecursive(t, dir)
	doc, err := vault.ResolveDocument("features/note")
	if err != nil {
		t.Fatalf("ResolveDocument error = %v", err)
	}
	got := resolvedTargets(doc)
	want := []string{"shared", "goals/login", "goals/login"}
	if len(got) != len(want) {
		t.Fatalf("resolved = %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("resolved[%d] = %q, want %q (all: %v)", i, got[i], want[i], got)
		}
	}
}

func TestRecursiveIgnoresHiddenAndVendorDirs(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "real.md", "# Real\n")
	writeFileAt(t, dir, ".git/HEAD.md", "# git\n")
	writeFileAt(t, dir, "node_modules/pkg/readme.md", "# vendored\n")
	writeFileAt(t, dir, ".obsidian/cfg.md", "# config\n")

	vault := loadRecursive(t, dir)
	if len(vault.Documents) != 1 || vault.Documents[0].RelPath != "real" {
		t.Fatalf("ignored dirs leaked: %d docs", len(vault.Documents))
	}
}

func TestRecursiveRenameSameDirRewritesLinks(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "goals/login.md", "---\ntype: goal\ntitle: login\n---\n# Login\n")
	writeFileAt(t, dir, "features/login.md",
		"# Feature\n\n[covers](../goals/login.md)\nsee [[goals/login]]\n")

	vault := loadRecursive(t, dir)
	result, err := vault.Rename("goals/login", "auth")
	if err != nil {
		t.Fatalf("Rename error = %v", err)
	}
	if result.OldName != "goals/login" || result.NewName != "goals/auth" {
		t.Fatalf("result names = %q -> %q, want goals/login -> goals/auth", result.OldName, result.NewName)
	}
	if !result.TitleUpdated {
		t.Fatalf("expected title updated")
	}
	if _, err := os.Stat(filepath.Join(dir, "goals", "auth.md")); err != nil {
		t.Fatalf("renamed file missing: %v", err)
	}
	feature := readFile(t, filepath.Join(dir, "features", "login.md"))
	if !strings.Contains(feature, "(../goals/auth.md)") || !strings.Contains(feature, "[[goals/auth]]") {
		t.Fatalf("links not rewritten in same dir:\n%s", feature)
	}
}

func TestRecursiveRenameCrossDirRecomputesRelativePaths(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "goals/login.md", "---\ntype: goal\n---\n# Login\n")
	writeFileAt(t, dir, "features/login.md", "# Feature\n\n[covers](../goals/login.md)\n")

	vault := loadRecursive(t, dir)
	result, err := vault.Rename("goals/login", "archive/auth")
	if err != nil {
		t.Fatalf("Rename error = %v", err)
	}
	if result.NewName != "archive/auth" {
		t.Fatalf("NewName = %q, want archive/auth", result.NewName)
	}
	if _, err := os.Stat(filepath.Join(dir, "archive", "auth.md")); err != nil {
		t.Fatalf("moved file missing (MkdirAll failed?): %v", err)
	}
	feature := readFile(t, filepath.Join(dir, "features", "login.md"))
	if !strings.Contains(feature, "(../archive/auth.md)") {
		t.Fatalf("cross-dir relative path not recomputed:\n%s", feature)
	}
}

func TestRecursiveRenameRejectsEscapingRoot(t *testing.T) {
	dir := t.TempDir()
	writeFileAt(t, dir, "goals/login.md", "# Login\n")
	vault := loadRecursive(t, dir)
	if _, err := vault.Rename("goals/login", "../escape"); err == nil {
		t.Fatalf("expected error renaming outside the vault root")
	}
}

// TestFlatIsSpecialCaseOfRecursive asserts that a single-directory vault loaded
// recursively resolves links identically to flat mode, proving the new path
// rules are a strict superset of the basename rules.
func TestFlatIsSpecialCaseOfRecursive(t *testing.T) {
	build := func() string {
		dir := t.TempDir()
		writeFile(t, filepath.Join(dir, "Alpha.md"), "# Alpha\n\n[[Beta]]\n[g](Gamma.md)\n")
		writeFile(t, filepath.Join(dir, "Beta.md"), "# Beta\n\n[[Alpha]]\n")
		writeFile(t, filepath.Join(dir, "Gamma.md"), "# Gamma\n")
		return dir
	}

	flat, err := Load(build())
	if err != nil {
		t.Fatalf("flat Load error = %v", err)
	}
	rec := loadRecursive(t, build())

	for _, name := range []string{"Alpha", "Beta", "Gamma"} {
		fd, err := flat.ResolveDocument(name)
		if err != nil {
			t.Fatalf("flat resolve %s: %v", name, err)
		}
		rd, err := rec.ResolveDocument(name)
		if err != nil {
			t.Fatalf("recursive resolve %s: %v", name, err)
		}
		if fd.Name != rd.Name || fd.Key != rd.Key {
			t.Fatalf("identity mismatch for %s: flat(%q,%q) vs rec(%q,%q)", name, fd.Name, fd.Key, rd.Name, rd.Key)
		}
		if a, b := resolvedTargets(fd), resolvedTargets(rd); !equalStrings(a, b) {
			t.Fatalf("resolved targets differ for %s: flat %v vs rec %v", name, a, b)
		}
	}
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
