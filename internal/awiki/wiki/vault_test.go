package wiki

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"golang.org/x/text/unicode/norm"
)

func TestLoadParsesFrontMatterAndLinks(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), `---
title: Alpha Title
aliases:
  - First Alias
---
[[Beta]]
[gamma](Gamma.md)
[[Delta|delta label]]
`+"```md\n[[Ignored]]\n```\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "[[Alpha]]\n")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "")
	writeFile(t, filepath.Join(dir, "Delta.md"), "")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	alpha, err := vault.ResolveDocument("First Alias")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}
	if alpha.Name != "Alpha" {
		t.Fatalf("ResolveDocument() = %q, want %q", alpha.Name, "Alpha")
	}
	if len(alpha.Links) != 3 {
		t.Fatalf("len(alpha.Links) = %d, want 3", len(alpha.Links))
	}
	if alpha.Links[0].Resolved != "Beta" || alpha.Links[1].Resolved != "Gamma" || alpha.Links[2].Resolved != "Delta" {
		t.Fatalf("resolved links = %#v, want Beta/Gamma/Delta", alpha.Links)
	}
}

func TestLoadParsesFrontMatterLinks(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), `---
title: Alpha
up: '[[Beta]]'
related: '[[Gamma|gamma label]]'
source: '[delta](Delta.md#Section)'
---
`)
	writeFile(t, filepath.Join(dir, "Beta.md"), "")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "")
	writeFile(t, filepath.Join(dir, "Delta.md"), "")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	alpha, err := vault.ResolveDocument("Alpha")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}
	if len(alpha.Links) != 3 {
		t.Fatalf("len(alpha.Links) = %d, want 3", len(alpha.Links))
	}
	if alpha.Links[0].Resolved != "Beta" || alpha.Links[1].Resolved != "Gamma" || alpha.Links[2].Resolved != "Delta" {
		t.Fatalf("resolved links = %#v, want Beta/Gamma/Delta", alpha.Links)
	}
}

func TestResolveDocumentAcceptsTitleAndAlias(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), `---
title: Alpha Title
aliases:
  - Alpha Alias
---
Alpha summary.
`)

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	doc, err := vault.ResolveDocument("Alpha Title")
	if err != nil {
		t.Fatalf("ResolveDocument(Alpha Title) error = %v", err)
	}
	if doc.Name != "Alpha" {
		t.Fatalf("ResolveDocument(Alpha Title) = %q, want %q", doc.Name, "Alpha")
	}

	doc, err = vault.ResolveDocument("Alpha Alias")
	if err != nil {
		t.Fatalf("ResolveDocument(Alpha Alias) error = %v", err)
	}
	if doc.Name != "Alpha" {
		t.Fatalf("ResolveDocument(Alpha Alias) = %q, want %q", doc.Name, "Alpha")
	}
}

func TestLinksDoNotResolveByTitleOrAlias(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), `---
title: Alpha Title
aliases:
  - Alpha Alias
---
Alpha summary.
`)
	writeFile(t, filepath.Join(dir, "RefByTitle.md"), "[[Alpha Title]]\n")
	writeFile(t, filepath.Join(dir, "RefByAlias.md"), "[[Alpha Alias]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	if inbound := vault.InboundNames("Alpha"); len(inbound) != 0 {
		t.Fatalf("InboundNames(Alpha) = %#v, want []", inbound)
	}

	outbound := vault.OutboundSummaries("RefByTitle")
	if len(outbound) != 1 || outbound[0].Name != "Alpha Title" || !outbound[0].Missing {
		t.Fatalf("OutboundSummaries(RefByTitle) = %#v, want missing Alpha Title", outbound)
	}

	outbound = vault.OutboundSummaries("RefByAlias")
	if len(outbound) != 1 || outbound[0].Name != "Alpha Alias" || !outbound[0].Missing {
		t.Fatalf("OutboundSummaries(RefByAlias) = %#v, want missing Alpha Alias", outbound)
	}
}

func TestCanonicalNameBeatsConflictingTitle(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "")
	writeFile(t, filepath.Join(dir, "Beta.md"), `---
title: Alpha
---
`)
	writeFile(t, filepath.Join(dir, "Ref.md"), "[[Alpha]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	doc, err := vault.ResolveDocument("Alpha")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}
	if doc.Name != "Alpha" {
		t.Fatalf("ResolveDocument() = %q, want %q", doc.Name, "Alpha")
	}

	inbound := vault.InboundNames("Alpha")
	if len(inbound) != 1 || inbound[0] != "Ref" {
		t.Fatalf("InboundNames() = %#v, want [Ref]", inbound)
	}

	if inbound := vault.InboundNames("Beta"); len(inbound) != 0 {
		t.Fatalf("InboundNames(Beta) = %#v, want []", inbound)
	}
}

func TestLoadMatchesUnicodeNormalizedNames(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, norm.NFD.String("홍익대학교")+".md"), "")
	writeFile(t, filepath.Join(dir, "Journal 2009-05.md"), "[[홍익대학교]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	doc, err := vault.ResolveDocument("홍익대학교")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}

	inbound := vault.InboundNames("홍익대학교")
	if len(inbound) != 1 || inbound[0] != "Journal 2009-05" {
		t.Fatalf("InboundNames() = %#v, want [Journal 2009-05]", inbound)
	}

	if report := vault.Lint(); contains(report.Orphans, doc.Name) {
		t.Fatalf("orphans = %#v, want %q to be linked", report.Orphans, doc.Name)
	}
}

func TestLoadUsesPersistentCacheForUnchangedDocs(t *testing.T) {
	useTempCacheDir(t)

	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "Alpha summary.\n\n[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n")

	_, stats, err := loadVault(dir, true)
	if err != nil {
		t.Fatalf("first loadVault() error = %v", err)
	}
	if stats.ParsedDocs != 2 || stats.CachedDocs != 0 {
		t.Fatalf("first load stats = %#v, want Parsed=2 Cached=0", stats)
	}

	cachePath, err := cacheFilePath(filepath.Clean(dir))
	if err != nil {
		t.Fatalf("cacheFilePath() error = %v", err)
	}
	if _, err := os.Stat(cachePath); err != nil {
		t.Fatalf("cache file stat error = %v", err)
	}

	vault, stats, err := loadVault(dir, true)
	if err != nil {
		t.Fatalf("second loadVault() error = %v", err)
	}
	if stats.ParsedDocs != 0 || stats.CachedDocs != 2 {
		t.Fatalf("second load stats = %#v, want Parsed=0 Cached=2", stats)
	}

	inbound := vault.InboundNames("Beta")
	if len(inbound) != 1 || inbound[0] != "Alpha" {
		t.Fatalf("InboundNames() = %#v, want [Alpha]", inbound)
	}
	alpha, err := vault.ResolveDocument("Alpha")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}
	if alpha.Excerpt != "Alpha summary." {
		t.Fatalf("Excerpt = %q, want %q", alpha.Excerpt, "Alpha summary.")
	}
}

func TestLoadReparsesOnlyChangedDocuments(t *testing.T) {
	useTempCacheDir(t)

	dir := t.TempDir()
	alphaPath := filepath.Join(dir, "Alpha.md")
	writeFile(t, alphaPath, "Old summary.\n\n[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "Beta summary.\n")

	if _, _, err := loadVault(dir, true); err != nil {
		t.Fatalf("initial loadVault() error = %v", err)
	}

	info, err := os.Stat(alphaPath)
	if err != nil {
		t.Fatalf("Stat() error = %v", err)
	}
	newContent := "New summary.\n\n[[Gamma]]\n"
	if err := os.WriteFile(alphaPath, []byte(newContent), 0o644); err != nil {
		t.Fatalf("write changed Alpha.md: %v", err)
	}
	nextTime := info.ModTime().Add(time.Second)
	if err := os.Chtimes(alphaPath, nextTime, nextTime); err != nil {
		t.Fatalf("Chtimes() error = %v", err)
	}
	writeFile(t, filepath.Join(dir, "Gamma.md"), "")

	vault, stats, err := loadVault(dir, true)
	if err != nil {
		t.Fatalf("changed loadVault() error = %v", err)
	}
	if stats.ParsedDocs != 2 || stats.CachedDocs != 1 {
		t.Fatalf("changed load stats = %#v, want Parsed=2 Cached=1", stats)
	}

	outbound := vault.OutboundSummaries("Alpha")
	if len(outbound) != 1 || outbound[0].Name != "Gamma" || outbound[0].Missing {
		t.Fatalf("OutboundSummaries() = %#v, want resolved Gamma", outbound)
	}
	alpha, err := vault.ResolveDocument("Alpha")
	if err != nil {
		t.Fatalf("ResolveDocument() error = %v", err)
	}
	if alpha.Excerpt != "New summary." {
		t.Fatalf("Excerpt = %q, want %q", alpha.Excerpt, "New summary.")
	}
}

func TestFirstPreviewLineSkipsFrontMatterAndHeading(t *testing.T) {
	content := `---
title: Sample
---

# Heading

First paragraph with [[Link]].

Second paragraph.
`

	got := FirstPreviewLine(content)
	want := "First paragraph with [[Link]]."
	if got != want {
		t.Fatalf("FirstPreviewLine() = %q, want %q", got, want)
	}
}

func TestFirstPreviewLineUsesFirstVisibleLine(t *testing.T) {
	content := `---
title: Sample
---

First line only
continues here on the next line

Second paragraph.
`

	got := FirstPreviewLine(content)
	want := "First line only"
	if got != want {
		t.Fatalf("FirstPreviewLine() = %q, want %q", got, want)
	}
}

func TestApproximateAverageShortestPath(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "[[Gamma]]\n")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "[[Delta]]\n")
	writeFile(t, filepath.Join(dir, "Delta.md"), "")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	report, err := vault.ApproximateAverageShortestPath(10, 1, 1)
	if err != nil {
		t.Fatalf("ApproximateAverageShortestPath() error = %v", err)
	}

	if report.ComponentSize != 4 {
		t.Fatalf("ComponentSize = %d, want 4", report.ComponentSize)
	}
	if report.SampleCount != 6 {
		t.Fatalf("SampleCount = %d, want 6", report.SampleCount)
	}
	if report.Average != 10.0/6.0 {
		t.Fatalf("Average = %v, want %v", report.Average, 10.0/6.0)
	}
	if len(report.LongerPaths) != 1 {
		t.Fatalf("len(LongerPaths) = %d, want 1", len(report.LongerPaths))
	}
	if got := report.LongerPaths[0]; got.Length != 3 || strings.Join(got.Nodes, " -> ") != "Alpha -> Beta -> Gamma -> Delta" {
		t.Fatalf("LongerPaths[0] = %#v, want Alpha..Delta length 3", got)
	}
}

func TestSplitWikiLinkParts(t *testing.T) {
	tests := []struct {
		name           string
		inner          string
		wantTarget     string
		wantLabel      string
		wantHasLabel   bool
		wantEscapedSep bool
	}{
		{
			name:           "plain target",
			inner:          "Almond",
			wantTarget:     "Almond",
			wantHasLabel:   false,
			wantEscapedSep: false,
		},
		{
			name:           "plain alias",
			inner:          "Almond|아몬드",
			wantTarget:     "Almond",
			wantLabel:      "아몬드",
			wantHasLabel:   true,
			wantEscapedSep: false,
		},
		{
			name:           "escaped alias separator",
			inner:          `Almond\|아몬드`,
			wantTarget:     "Almond",
			wantLabel:      "아몬드",
			wantHasLabel:   true,
			wantEscapedSep: true,
		},
		{
			name:           "escaped alias separator with heading",
			inner:          `Old#Section\|self`,
			wantTarget:     "Old#Section",
			wantLabel:      "self",
			wantHasLabel:   true,
			wantEscapedSep: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			target, label, hasLabel, escapedSep := splitWikiLinkParts(tt.inner)
			if target != tt.wantTarget || label != tt.wantLabel || hasLabel != tt.wantHasLabel || escapedSep != tt.wantEscapedSep {
				t.Fatalf("splitWikiLinkParts(%q) = (%q, %q, %v, %v), want (%q, %q, %v, %v)",
					tt.inner, target, label, hasLabel, escapedSep,
					tt.wantTarget, tt.wantLabel, tt.wantHasLabel, tt.wantEscapedSep,
				)
			}
		})
	}
}

func TestLoadParsesEscapedWikiLinkAliasInTable(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Fat in foods.md"), "| [[Almond\\|아몬드]] |\n")
	writeFile(t, filepath.Join(dir, "Almond.md"), `---
aliases:
- 아몬드
---
`)

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	inbound := vault.InboundNames("Almond")
	if len(inbound) != 1 || inbound[0] != "Fat in foods" {
		t.Fatalf("InboundNames() = %#v, want [Fat in foods]", inbound)
	}
}

func TestLintFindsOrphansAndIslands(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "[[Delta]]\n")
	writeFile(t, filepath.Join(dir, "Delta.md"), "")
	writeFile(t, filepath.Join(dir, "Orphan.md"), "[[Missing]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	report := vault.Lint()
	if len(report.Orphans) != 1 || report.Orphans[0] != "Orphan" {
		t.Fatalf("orphans = %#v, want [Orphan]", report.Orphans)
	}
	if len(report.Islands) != 1 {
		t.Fatalf("islands = %#v, want 1 island", report.Islands)
	}
	if got := report.Islands[0]; len(got) != 2 || got[0] != "Delta" || got[1] != "Gamma" {
		t.Fatalf("island = %#v, want [Delta Gamma]", got)
	}
}

func TestLintReportsQualityMetrics(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "Alpha summary.\n\n[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "Gamma summary.\n\n[[Delta]]\n")
	writeFile(t, filepath.Join(dir, "Delta.md"), "Delta summary.\n")
	writeFile(t, filepath.Join(dir, "Orphan.md"), "[[Missing]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	report := vault.Lint()
	if report.DocumentCount != 5 {
		t.Fatalf("DocumentCount = %d, want 5", report.DocumentCount)
	}
	if report.LargestComponentSize != 2 {
		t.Fatalf("LargestComponentSize = %d, want 2", report.LargestComponentSize)
	}
	if report.CoveredDocuments != 4 {
		t.Fatalf("CoveredDocuments = %d, want 4", report.CoveredDocuments)
	}
	if report.LargestComponentRatio() != 2.0/5.0 {
		t.Fatalf("LargestComponentRatio() = %v, want %v", report.LargestComponentRatio(), 2.0/5.0)
	}
	if report.OrphanRate() != 1.0/5.0 {
		t.Fatalf("OrphanRate() = %v, want %v", report.OrphanRate(), 1.0/5.0)
	}
	if report.ContentCoverage() != 4.0/5.0 {
		t.Fatalf("ContentCoverage() = %v, want %v", report.ContentCoverage(), 4.0/5.0)
	}
}

func TestShortestPathUsesUndirectedGraph(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Alpha.md"), "[[Beta]]\n")
	writeFile(t, filepath.Join(dir, "Beta.md"), "")
	writeFile(t, filepath.Join(dir, "Gamma.md"), "[[Beta]]\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	path, err := vault.ShortestPath("Alpha", "Gamma")
	if err != nil {
		t.Fatalf("ShortestPath() error = %v", err)
	}

	want := []string{"Alpha", "Beta", "Gamma"}
	for i := range want {
		if path[i] != want[i] {
			t.Fatalf("path[%d] = %q, want %q", i, path[i], want[i])
		}
	}
	if dir := vault.EdgeDirection("Beta", "Gamma"); dir != "<-" {
		t.Fatalf("EdgeDirection(Beta, Gamma) = %q, want %q", dir, "<-")
	}
}

func TestWantedPagesRanksMissingTargetsAndKeepsLineContext(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Doc1.md"), "First paragraph with [[Wanted A]].\ncontinues here.\n\nAnother paragraph with [[Wanted B]].\n")
	writeFile(t, filepath.Join(dir, "Doc2.md"), "Another paragraph with [[Wanted A]] and [[Wanted A]] again.\n")

	vault, err := Load(dir)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	pages := vault.WantedPages(10)
	if len(pages) != 2 {
		t.Fatalf("len(WantedPages()) = %d, want 2", len(pages))
	}

	first := pages[0]
	if first.Name != "Wanted A" || first.Mentions != 3 || first.SourceDocuments != 2 {
		t.Fatalf("WantedPages()[0] = %#v, want Wanted A mentions=3 source_documents=2", first)
	}
	if len(first.Sources) != 2 {
		t.Fatalf("len(WantedPages()[0].Sources) = %d, want 2", len(first.Sources))
	}
	if first.Sources[0].Document != "Doc2" || first.Sources[0].Context != "Another paragraph with [[Wanted A]] and [[Wanted A]] again." {
		t.Fatalf("WantedPages()[0].Sources[0] = %#v", first.Sources[0])
	}
	if first.Sources[1].Document != "Doc1" || first.Sources[1].Context != "First paragraph with [[Wanted A]]." {
		t.Fatalf("WantedPages()[0].Sources[1] = %#v", first.Sources[1])
	}

	second := pages[1]
	if second.Name != "Wanted B" || second.Mentions != 1 || second.SourceDocuments != 1 {
		t.Fatalf("WantedPages()[1] = %#v, want Wanted B mentions=1 source_documents=1", second)
	}
	if len(second.Sources) != 1 || second.Sources[0].Document != "Doc1" || second.Sources[0].Context != "Another paragraph with [[Wanted B]]." {
		t.Fatalf("WantedPages()[1].Sources = %#v", second.Sources)
	}
}

func TestRenameUpdatesLinksAndTitle(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Old.md"), `---
title: Old
---
[[Old#Section|self]]
`)
	writeFile(t, filepath.Join(dir, "Ref.md"), "[[Old]]\n[[Old|alias]]\n[text](Old.md#Section)\n![image](Old.md)\n")

	result, err := Rename(dir, "Old", "New")
	if err != nil {
		t.Fatalf("Rename() error = %v", err)
	}
	if result.LinksUpdated != 4 {
		t.Fatalf("LinksUpdated = %d, want 4", result.LinksUpdated)
	}
	if !result.TitleUpdated {
		t.Fatalf("TitleUpdated = false, want true")
	}

	if _, err := os.Stat(filepath.Join(dir, "Old.md")); !os.IsNotExist(err) {
		t.Fatalf("Old.md still exists or stat failed: %v", err)
	}

	newContent := readFile(t, filepath.Join(dir, "New.md"))
	if !containsAll(newContent, "title: New", "[[New#Section|self]]") {
		t.Fatalf("New.md content = %q", newContent)
	}

	refContent := readFile(t, filepath.Join(dir, "Ref.md"))
	if !containsAll(refContent, "[[New]]", "[[New|alias]]", "[text](New.md#Section)", "![image](Old.md)") {
		t.Fatalf("Ref.md content = %q", refContent)
	}
}

func TestRenameUpdatesFrontMatterLinks(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Old.md"), `---
title: Old
up: '[[Old#Section|self]]'
source: '[text](Old.md#Section)'
---
`)
	writeFile(t, filepath.Join(dir, "Ref.md"), `---
prev: '[[Old]]'
related: '[alias](Old.md)'
---
[[Old]]
`)

	result, err := Rename(dir, "Old", "New")
	if err != nil {
		t.Fatalf("Rename() error = %v", err)
	}
	if result.LinksUpdated != 5 {
		t.Fatalf("LinksUpdated = %d, want 5", result.LinksUpdated)
	}
	if !result.TitleUpdated {
		t.Fatalf("TitleUpdated = false, want true")
	}

	newContent := readFile(t, filepath.Join(dir, "New.md"))
	if !containsAll(newContent, "title: New", "up: '[[New#Section|self]]'", "source: '[text](New.md#Section)'") {
		t.Fatalf("New.md content = %q", newContent)
	}

	refContent := readFile(t, filepath.Join(dir, "Ref.md"))
	if !containsAll(refContent, "prev: '[[New]]'", "related: '[alias](New.md)'", "[[New]]") {
		t.Fatalf("Ref.md content = %q", refContent)
	}
}

func TestRenamePreservesEscapedWikiLinkAliasSeparator(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "Old.md"), "")
	writeFile(t, filepath.Join(dir, "Ref.md"), "| [[Old\\|alias]] |\n")

	result, err := Rename(dir, "Old", "New")
	if err != nil {
		t.Fatalf("Rename() error = %v", err)
	}
	if result.LinksUpdated != 1 {
		t.Fatalf("LinksUpdated = %d, want 1", result.LinksUpdated)
	}

	refContent := readFile(t, filepath.Join(dir, "Ref.md"))
	if !containsAll(refContent, `[[New\|alias]]`) {
		t.Fatalf("Ref.md content = %q", refContent)
	}
}

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}

func readFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(data)
}

func containsAll(content string, parts ...string) bool {
	for _, part := range parts {
		if !strings.Contains(content, part) {
			return false
		}
	}
	return true
}

func contains(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func useTempCacheDir(t testing.TB) {
	t.Helper()

	dir := t.TempDir()
	previous := userCacheDir
	userCacheDir = func() (string, error) {
		return dir, nil
	}
	t.Cleanup(func() {
		userCacheDir = previous
	})
}
