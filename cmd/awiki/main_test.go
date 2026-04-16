package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/corca-ai/awiki/internal/awiki/wiki"
)

func TestHasHelpFlag(t *testing.T) {
	tests := []struct {
		name string
		args []string
		want bool
	}{
		{"empty", nil, false},
		{"no help", []string{"lint", "-root", "wiki"}, false},
		{"-help", []string{"-help"}, true},
		{"--help", []string{"--help"}, true},
		{"-h", []string{"-h"}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := hasHelpFlag(tt.args)
			if got != tt.want {
				t.Fatalf("hasHelpFlag(%v) = %v, want %v", tt.args, got, tt.want)
			}
		})
	}
}

func TestPrintCommandErrorStructured(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	printCommandError(errors.New("// lint_failed orphans=1 islands=0\n[[Orphan]]: Orphan summary."))

	want := "// lint_failed orphans=1 islands=0\n[[Orphan]]: Orphan summary.\n"
	if errOut.String() != want {
		t.Fatalf("printCommandError() output = %q, want %q", errOut.String(), want)
	}
}

func TestPrintCommandErrorPlain(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	printCommandError(errors.New("boom"))

	want := "awiki: boom\n"
	if errOut.String() != want {
		t.Fatalf("printCommandError() output = %q, want %q", errOut.String(), want)
	}
}

func TestUsageIncludesCommandArguments(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	usage()

	want := "" +
		"awiki helps maintain the quality of a flat-file Markdown wiki.\n" +
		"Quality here means keeping notes well connected, reducing orphans and disconnected islands, keeping most pages inside one large component,\n" +
		"avoiding empty stubs, and making long paths and heavily linked missing pages easy to inspect.\n" +
		"\n" +
		"Commands:\n" +
		"  awiki lint [flags]\n" +
		"      Validate the wiki graph\n" +
		"  awiki avg-shortest-path [flags]\n" +
		"      Estimate average shortest path length and print sampled long paths\n" +
		"  awiki path [flags] <from> <to>\n" +
		"      Print the shortest path between two documents\n" +
		"  awiki rename [flags] <old> <new>\n" +
		"      Rename a document and update links to it\n" +
		"  awiki links [flags] <document>\n" +
		"      Show inbound and outbound links for a document\n" +
		"  awiki wanted [flags]\n" +
		"      Show the most-linked missing pages\n" +
		"\n" +
		"Examples:\n" +
		"  awiki path \"The China study (book)\" \"What to Eat\"\n" +
		"  awiki links \"Books Ive read\"\n" +
		"\n" +
		"Use `awiki <command> -h` for command-specific help.\n"
	if errOut.String() != want {
		t.Fatalf("usage() output = %q, want %q", errOut.String(), want)
	}
}

func TestPathHelpShowsQuotedExample(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	if err := pathCmd([]string{"-h"}); err != nil {
		t.Fatalf("pathCmd(-h) error = %v", err)
	}

	got := errOut.String()
	wantParts := []string{
		"Usage: awiki path [flags] <from> <to>",
		"Quote document names that contain spaces.",
		"awiki path \"The China study (book)\" \"What to Eat\"",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("path help missing %q in %q", want, got)
		}
	}
}

func TestRenameHelpShowsQuotedExample(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	if err := renameCmd([]string{"-h"}); err != nil {
		t.Fatalf("renameCmd(-h) error = %v", err)
	}

	got := errOut.String()
	wantParts := []string{
		"Usage: awiki rename [flags] <old> <new>",
		"Quote document names that contain spaces.",
		"awiki rename \"Old Note\" \"New Note\"",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("rename help missing %q in %q", want, got)
		}
	}
}

func TestLinksHelpShowsQuotedExample(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	if err := linksCmd([]string{"-h"}); err != nil {
		t.Fatalf("linksCmd(-h) error = %v", err)
	}

	got := errOut.String()
	wantParts := []string{
		"Usage: awiki links [flags] <document>",
		"Quote document names that contain spaces.",
		"awiki links \"Books Ive read\"",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("links help missing %q in %q", want, got)
		}
	}
}

func TestAvgShortestPathHelpMentionsLongPaths(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	if err := avgShortestPathCmd([]string{"-h"}); err != nil {
		t.Fatalf("avgShortestPathCmd(-h) error = %v", err)
	}

	got := errOut.String()
	wantParts := []string{
		"Usage: awiki avg-shortest-path [flags]",
		"Estimate the average shortest path length on the largest connected component",
		"and print sampled longer-than-average paths.",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("avg-shortest-path help missing %q in %q", want, got)
		}
	}
}

func TestWantedHelpMentionsParagraphs(t *testing.T) {
	oldStderr := stderr
	var errOut bytes.Buffer
	stderr = &errOut
	t.Cleanup(func() {
		stderr = oldStderr
	})

	if err := wantedCmd([]string{"-h"}); err != nil {
		t.Fatalf("wantedCmd(-h) error = %v", err)
	}

	got := errOut.String()
	wantParts := []string{
		"Usage: awiki wanted [flags]",
		"Show the most-linked missing pages and the lines that reference them.",
		"awiki wanted -n 10",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("wanted help missing %q in %q", want, got)
		}
	}
}

func TestFormatLintReport(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Orphan.md"), "Orphan summary.\n")
	writeTestFile(t, filepath.Join(dir, "IslandA.md"), "Island A summary.\n")
	writeTestFile(t, filepath.Join(dir, "IslandB.md"), "Island B summary.\n")

	vault, err := wiki.Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	report := wiki.LintReport{
		DocumentCount:        3,
		LargestComponentSize: 2,
		CoveredDocuments:     3,
		Orphans:              []string{"Orphan"},
		Islands:              [][]string{{"IslandA", "IslandB"}},
	}

	got := formatLintReport(vault, report)
	want := "// lint_failed documents=3 orphans=1 islands=1 largest_component_ratio=0.6667 orphan_rate=0.3333 content_coverage=1.0000\n// orphan\n[[Orphan]]: Orphan summary.\n// island=1\n[[IslandA]]: Island A summary.\n[[IslandB]]: Island B summary."
	if got != want {
		t.Fatalf("formatLintReport() = %q, want %q", got, want)
	}
}

func TestLintCmdPrintsMetricsOnSuccess(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Alpha.md"), "Alpha summary.\n\n[[Beta]]\n")
	writeTestFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := lintCmd([]string{"-root", dir}); err != nil {
		t.Fatalf("lintCmd() error = %v", err)
	}

	want := "// ok connected_graph documents=2 largest_component_ratio=1.0000 orphan_rate=0.0000 content_coverage=1.0000\n"
	if out.String() != want {
		t.Fatalf("lintCmd() output = %q, want %q", out.String(), want)
	}
}

func TestLinksCmd(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Alpha.md"), "---\ntitle: Alpha\n---\n\n# Alpha\n\nAlpha summary.\n\n[[Beta]]\n[[Missing]]\n")
	writeTestFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n\n[[Alpha]]\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := linksCmd([]string{"-root", dir, "Alpha"}); err != nil {
		t.Fatalf("linksCmd() error = %v", err)
	}

	want := "// this page\n[[Alpha]]: Alpha summary.\n// incoming links\n[[Beta]]: Beta summary.\n// outgoing links\n[[Beta]]: Beta summary.\n[[Missing]]: (missing)\n"
	if out.String() != want {
		t.Fatalf("linksCmd() output = %q, want %q", out.String(), want)
	}
}

func TestPathCmd(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Alpha.md"), "Alpha summary.\n\n[[Beta]]\n")
	writeTestFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n")
	writeTestFile(t, filepath.Join(dir, "Gamma.md"), "Gamma summary.\n\n[[Beta]]\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := pathCmd([]string{"-root", dir, "Alpha", "Gamma"}); err != nil {
		t.Fatalf("pathCmd() error = %v", err)
	}

	want := "[[Alpha]]: Alpha summary.\n[[Beta]]: Beta summary.\n[[Gamma]]: Gamma summary.\n"
	if out.String() != want {
		t.Fatalf("pathCmd() output = %q, want %q", out.String(), want)
	}
}

func TestPathCmdPrintsEmptyForBlankDocument(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Alpha.md"), "[[Beta]]\n")
	writeTestFile(t, filepath.Join(dir, "Beta.md"), "")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := pathCmd([]string{"-root", dir, "Alpha", "Beta"}); err != nil {
		t.Fatalf("pathCmd() error = %v", err)
	}

	want := "[[Alpha]]: [[Beta]]\n[[Beta]]: (empty)\n"
	if out.String() != want {
		t.Fatalf("pathCmd() output = %q, want %q", out.String(), want)
	}
}

func TestAvgShortestPathCmd(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Alpha.md"), "Alpha summary.\n\n[[Beta]]\n")
	writeTestFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n\n[[Gamma]]\n")
	writeTestFile(t, filepath.Join(dir, "Gamma.md"), "Gamma summary.\n\n[[Delta]]\n")
	writeTestFile(t, filepath.Join(dir, "Delta.md"), "Delta summary.\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := avgShortestPathCmd([]string{"-root", dir, "-samples", "10", "-examples", "1", "-seed", "1"}); err != nil {
		t.Fatalf("avgShortestPathCmd() error = %v", err)
	}

	got := out.String()
	wantParts := []string{
		"// largest_component_size=4 samples=6 average_shortest_path=1.6667",
		"[[Alpha]]: Alpha summary.",
		"[[Beta]]: Beta summary.",
		"[[Gamma]]: Gamma summary.",
		"[[Delta]]: Delta summary.",
	}
	for _, want := range wantParts {
		if !strings.Contains(got, want) {
			t.Fatalf("avgShortestPathCmd() output missing %q in %q", want, got)
		}
	}
	if strings.Contains(got, "example ") {
		t.Fatalf("avgShortestPathCmd() output should not contain example headers: %q", got)
	}
}

func TestAvgShortestPathCmdDoesNotTruncateLongPaths(t *testing.T) {
	dir := t.TempDir()
	docs := []string{"Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota"}
	for i, name := range docs {
		content := name + " summary.\n"
		if i < len(docs)-1 {
			content += "\n[[" + docs[i+1] + "]]\n"
		}
		writeTestFile(t, filepath.Join(dir, name+".md"), content)
	}

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := avgShortestPathCmd([]string{"-root", dir, "-samples", "100", "-examples", "1", "-seed", "1"}); err != nil {
		t.Fatalf("avgShortestPathCmd() error = %v", err)
	}

	got := out.String()
	if strings.Contains(got, "(중략)") {
		t.Fatalf("avgShortestPathCmd() output should not truncate long paths: %q", got)
	}
	for _, name := range docs {
		want := "[[" + name + "]]"
		if !strings.Contains(got, want) {
			t.Fatalf("avgShortestPathCmd() output missing %q in %q", want, got)
		}
	}
}

func TestWantedCmd(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Doc1.md"), "First paragraph with [[Wanted A]].\ncontinues here.\n\nAnother paragraph with [[Wanted B]].\n")
	writeTestFile(t, filepath.Join(dir, "Doc2.md"), "Another paragraph with [[Wanted A]] and [[Wanted A]] again.\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := wantedCmd([]string{"-root", dir}); err != nil {
		t.Fatalf("wantedCmd() error = %v", err)
	}

	want := "" +
		"[[Wanted A]] (3 links)\n" +
		"\n" +
		"- [[Doc2]]: Another paragraph with [[Wanted A]] and [[Wanted A]] again.\n" +
		"- [[Doc1]]: First paragraph with [[Wanted A]].\n" +
		"\n" +
		"[[Wanted B]] (1 link)\n" +
		"\n" +
		"- [[Doc1]]: Another paragraph with [[Wanted B]].\n"
	if out.String() != want {
		t.Fatalf("wantedCmd() output = %q, want %q", out.String(), want)
	}
}

func TestWantedCmdTruncatesLongSourceLists(t *testing.T) {
	dir := t.TempDir()
	for i := 1; i <= wantedSourcePreviewLimit+1; i++ {
		name := fmt.Sprintf("Doc%02d", i)
		writeTestFile(t, filepath.Join(dir, name+".md"), fmt.Sprintf("[[Wanted A]] from %s.\n", name))
	}

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := wantedCmd([]string{"-root", dir}); err != nil {
		t.Fatalf("wantedCmd() error = %v", err)
	}

	got := out.String()
	if !strings.Contains(got, "[[Wanted A]] (11 links)\n\n") {
		t.Fatalf("wantedCmd() output missing header in %q", got)
	}
	if !strings.Contains(got, "_ ...\n") {
		t.Fatalf("wantedCmd() output missing ellipsis in %q", got)
	}
	if strings.Count(got, "- [[Doc") != wantedSourcePreviewLimit {
		t.Fatalf("wantedCmd() printed %d source bullets, want %d in %q", strings.Count(got, "- [[Doc"), wantedSourcePreviewLimit, got)
	}
}

func TestRenameCmd(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "Old.md"), "---\ntitle: Old\n---\n\nOld summary.\n")

	oldStdout, oldStderr := stdout, stderr
	var out bytes.Buffer
	var errOut bytes.Buffer
	stdout = &out
	stderr = &errOut
	t.Cleanup(func() {
		stdout = oldStdout
		stderr = oldStderr
	})

	if err := renameCmd([]string{"-root", dir, "Old", "New"}); err != nil {
		t.Fatalf("renameCmd() error = %v", err)
	}

	want := "// rename old=Old.md new=New.md\n// links_updated=0 files_touched=1 title_updated=true\n"
	if out.String() != want {
		t.Fatalf("renameCmd() output = %q, want %q", out.String(), want)
	}
}

func TestDocumentPreviewUsesFullPreviewLine(t *testing.T) {
	doc := &wiki.Document{
		Name:    "Alpha",
		Excerpt: "First sentence. Second sentence.",
	}

	got := documentPreview(doc)
	want := "First sentence. Second sentence."
	if got != want {
		t.Fatalf("documentPreview() = %q, want %q", got, want)
	}
}

func TestDocumentPreviewDoesNotStopAtInitials(t *testing.T) {
	doc := &wiki.Document{
		Name:    "The China study (book)",
		Excerpt: "[[Colin Campbell|콜린 캠벨]]과 그의 아들 [[Thomas M. Campbell II|토머스 M. 캠벨 2세]]의 공저. 원제는 \"The China Study\"이다.",
	}

	got := documentPreview(doc)
	want := "[[Colin Campbell|콜린 캠벨]]과 그의 아들 [[Thomas M. Campbell II|토머스 M. 캠벨 2세]]의 공저. 원제는 \"The China Study\"이다."
	if got != want {
		t.Fatalf("documentPreview() = %q, want %q", got, want)
	}
}

func writeTestFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}
