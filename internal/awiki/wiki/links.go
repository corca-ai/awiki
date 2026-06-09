package wiki

import (
	"path"
	"regexp"
	"strings"

	"golang.org/x/text/unicode/norm"
)

type LinkKind string

const (
	LinkWiki     LinkKind = "wiki"
	LinkMarkdown LinkKind = "markdown"
)

type Link struct {
	Kind          LinkKind
	DisplayTarget string
	TargetKey     string
	// RawTarget is the literal link target with any #fragment removed and
	// surrounding <> trimmed (e.g. "../goals/g1.md", "folder/note", "Note").
	// It carries the directory information that TargetKey (a basename key)
	// discards, so recursive vaults can resolve links by repo-relative path.
	RawTarget string
	Resolved  string
	Context   string
}

var (
	wikiLinkRE     = regexp.MustCompile(`\[\[[^\]\n]+\]\]`)
	markdownLinkRE = regexp.MustCompile(`!?\[[^\]\n]*\]\(([^)\n]+)\)`)
)

func ParseLinks(content string) []Link {
	fm := ParseFrontMatter(content)
	if !fm.Present {
		return parseBodyLinks(scanLines(content))
	}

	links := parseFrontMatterLinks(scanLines(content[:fm.BodyOffset]))
	links = append(links, parseBodyLinks(scanLines(content[fm.BodyOffset:]))...)
	return links
}

func RewriteDocumentLinks(content, oldName, newName string) (rewritten string, changes int) {
	oldKey := documentKey(oldName)
	if oldKey == "" {
		return content, 0
	}

	fm := ParseFrontMatter(content)
	if !fm.Present {
		rewritten, changes := rewriteLinkLines(scanLines(content), oldKey, newName, true)
		if changes == 0 {
			return content, 0
		}
		return rewritten, changes
	}

	head, headChanges := rewriteLinkLines(scanLines(content[:fm.BodyOffset]), oldKey, newName, false)
	body, bodyChanges := rewriteLinkLines(scanLines(content[fm.BodyOffset:]), oldKey, newName, true)
	changes = headChanges + bodyChanges
	if changes == 0 {
		return content, 0
	}
	return head + body, changes
}

// rewriteDocumentLinksRecursive rewrites the links in a source document that
// resolve to target.doc, pointing them at target's new repo-relative path and
// recomputing relative link text. sourceRelDir is the source document's
// directory. newBaseUnique reports whether the new basename is unique vault-
// wide, so bare wikilinks can stay bare instead of becoming path-qualified.
func (v *Vault) rewriteDocumentLinksRecursive(content, sourceRelDir string, target renameTarget, newBaseUnique bool) (rewritten string, changes int) {
	fm := ParseFrontMatter(content)
	if !fm.Present {
		rewritten, changes = v.rewriteRecursiveLines(scanLines(content), sourceRelDir, target, newBaseUnique, true)
		if changes == 0 {
			return content, 0
		}
		return rewritten, changes
	}

	head, headChanges := v.rewriteRecursiveLines(scanLines(content[:fm.BodyOffset]), sourceRelDir, target, newBaseUnique, false)
	body, bodyChanges := v.rewriteRecursiveLines(scanLines(content[fm.BodyOffset:]), sourceRelDir, target, newBaseUnique, true)
	changes = headChanges + bodyChanges
	if changes == 0 {
		return content, 0
	}
	return head + body, changes
}

func (v *Vault) rewriteRecursiveLines(lines []textLine, sourceRelDir string, target renameTarget, newBaseUnique, respectFences bool) (rewritten string, changes int) {
	var (
		b          strings.Builder
		inFence    bool
		fenceRune  rune
		fenceWidth int
	)

	for _, line := range lines {
		if respectFences {
			trimmed := strings.TrimSpace(trimLine(line.text))
			if marker, width, ok := fenceStart(trimmed); ok {
				if inFence && marker == fenceRune && width >= fenceWidth {
					inFence = false
				} else if !inFence {
					inFence = true
					fenceRune = marker
					fenceWidth = width
				}
				b.WriteString(line.text)
				continue
			}
			if inFence {
				b.WriteString(line.text)
				continue
			}
		}

		updated := line.text
		var lineChanges int
		updated, lineChanges = v.rewriteWikiLinksRecursive(updated, sourceRelDir, target, newBaseUnique)
		changes += lineChanges
		updated, lineChanges = v.rewriteMarkdownLinksRecursive(updated, sourceRelDir, target)
		changes += lineChanges
		b.WriteString(updated)
	}

	return b.String(), changes
}

func (v *Vault) rewriteWikiLinksRecursive(line, sourceRelDir string, target renameTarget, newBaseUnique bool) (rewritten string, changes int) {
	matches := wikiLinkRE.FindAllStringIndex(maskInlineCode(line), -1)
	if len(matches) == 0 {
		return line, 0
	}

	var (
		b    strings.Builder
		last int
	)
	for _, match := range matches {
		start, end := match[0], match[1]
		b.WriteString(line[last:start])
		full := line[start:end]
		last = end

		replacement, ok := v.wikiLinkReplacement(full, sourceRelDir, target, newBaseUnique)
		if !ok || replacement == full {
			b.WriteString(full)
			continue
		}
		b.WriteString(replacement)
		changes++
	}
	b.WriteString(line[last:])

	return b.String(), changes
}

// wikiLinkReplacement returns the rewritten form of a wikilink whose target
// resolves to the renamed document, or ok=false when the link points elsewhere.
func (v *Vault) wikiLinkReplacement(full, sourceRelDir string, target renameTarget, newBaseUnique bool) (string, bool) {
	inner := full[2 : len(full)-2]
	targetPart, label, hasLabel, escapedSeparator := splitWikiLinkParts(inner)
	base, suffix := splitTargetSuffix(strings.TrimSpace(targetPart))
	raw := rawLinkTarget(base)

	cand, ok := v.resolveRecursive(sourceRelDir, LinkWiki, raw, documentKey(base))
	if !ok || cand != target.doc {
		return "", false
	}

	newTargetText := target.newRelPath
	if !strings.ContainsAny(raw, `/\`) && newBaseUnique {
		newTargetText = target.newBase
	}
	if strings.HasSuffix(strings.ToLower(strings.TrimSpace(base)), ".md") {
		newTargetText += ".md"
	}
	newTargetText += suffix

	if !hasLabel {
		return "[[" + newTargetText + "]]", true
	}
	separator := "|"
	if escapedSeparator {
		separator = `\|`
	}
	return "[[" + newTargetText + separator + label + "]]", true
}

func (v *Vault) rewriteMarkdownLinksRecursive(line, sourceRelDir string, target renameTarget) (rewritten string, changes int) {
	matches := markdownLinkRE.FindAllStringSubmatchIndex(maskInlineCode(line), -1)
	if len(matches) == 0 {
		return line, 0
	}

	var (
		b    strings.Builder
		last int
	)
	for _, match := range matches {
		start, end := match[0], match[1]
		destStart, destEnd := match[2], match[3]
		full := line[start:end]

		b.WriteString(line[last:start])
		if strings.HasPrefix(full, "!") {
			b.WriteString(full)
			last = end
			continue
		}

		newDest, changed := v.rewriteMarkdownDestRecursive(line[destStart:destEnd], sourceRelDir, target)
		if !changed {
			b.WriteString(full)
			last = end
			continue
		}

		b.WriteString(line[start:destStart])
		b.WriteString(newDest)
		b.WriteString(line[destEnd:end])
		changes++
		last = end
	}
	b.WriteString(line[last:])

	return b.String(), changes
}

func (v *Vault) rewriteMarkdownDestRecursive(dest, sourceRelDir string, target renameTarget) (string, bool) {
	pathStart, pathEnd, ok := destinationBounds(dest)
	if !ok {
		return dest, false
	}

	rawPath := dest[pathStart:pathEnd]
	base, suffix := splitTargetSuffix(rawPath)
	raw := rawLinkTarget(base)

	cand, ok := v.resolveRecursive(sourceRelDir, LinkMarkdown, raw, documentKey(base))
	if !ok || cand != target.doc {
		return dest, false
	}

	newRel := relPathFromDir(sourceRelDir, target.newRelPath)
	if strings.EqualFold(path.Ext(strings.TrimSpace(base)), ".md") {
		newRel += ".md"
	}
	newPath := newRel + suffix
	if newPath == rawPath {
		return dest, false
	}
	return dest[:pathStart] + newPath + dest[pathEnd:], true
}

func parseFrontMatterLinks(lines []textLine) []Link {
	var links []Link
	for _, line := range lines {
		lineLinks := parseLinksInLine(line.text)
		if len(lineLinks) == 0 {
			continue
		}

		context := normalizeLinkContext([]string{trimLine(line.text)})
		for i := range lineLinks {
			lineLinks[i].Context = context
		}
		links = append(links, lineLinks...)
	}
	return links
}

func parseBodyLinks(lines []textLine) []Link {
	var (
		links      []Link
		inFence    bool
		fenceRune  rune
		fenceWidth int
	)

	for _, line := range lines {
		lineLinks, nextInFence, nextFenceRune, nextFenceWidth := parseBodyLineLinks(
			line,
			inFence,
			fenceRune,
			fenceWidth,
		)
		inFence = nextInFence
		fenceRune = nextFenceRune
		fenceWidth = nextFenceWidth
		if len(lineLinks) == 0 {
			continue
		}
		links = append(links, lineLinks...)
	}

	return links
}

func parseBodyLineLinks(line textLine, inFence bool, fenceRune rune, fenceWidth int) (links []Link, nextInFence bool, nextFenceRune rune, nextFenceWidth int) {
	trimmed := strings.TrimSpace(trimLine(line.text))
	if marker, width, ok := fenceStart(trimmed); ok {
		nextInFence, nextFenceRune, nextFenceWidth = nextFenceState(inFence, fenceRune, fenceWidth, marker, width)
		return nil, nextInFence, nextFenceRune, nextFenceWidth
	}
	if inFence || trimmed == "" {
		return nil, inFence, fenceRune, fenceWidth
	}

	lineLinks := parseLinksInLine(line.text)
	if len(lineLinks) == 0 {
		return nil, inFence, fenceRune, fenceWidth
	}

	context := normalizeLinkContext([]string{trimmed})
	for i := range lineLinks {
		lineLinks[i].Context = context
	}
	return lineLinks, inFence, fenceRune, fenceWidth
}

func nextFenceState(inFence bool, fenceRune rune, fenceWidth int, marker rune, width int) (nextInFence bool, nextFenceRune rune, nextFenceWidth int) {
	if inFence && marker == fenceRune && width >= fenceWidth {
		return false, fenceRune, fenceWidth
	}
	if !inFence {
		return true, marker, width
	}
	return inFence, fenceRune, fenceWidth
}

func parseLinksInLine(line string) []Link {
	masked := maskInlineCode(line)
	links := parseWikiLinks(masked)
	links = append(links, parseMarkdownLinks(masked)...)
	return links
}

// maskInlineCode replaces the contents of inline code spans with spaces while
// preserving the line's byte length, so link extraction and rewriting ignore
// example link syntax shown as inline code (e.g. `[[Foo]]`). The backtick
// delimiters themselves are kept. A backtick run only opens a span when a
// matching closing run of equal length appears later on the same line;
// otherwise the backticks are treated as literal text.
func maskInlineCode(line string) string {
	if !strings.ContainsRune(line, '`') {
		return line
	}

	b := []byte(line)
	for i := 0; i < len(b); {
		if b[i] != '`' {
			i++
			continue
		}

		contentStart := backtickRunEnd(b, i)
		openLen := contentStart - i
		closeStart, closeEnd, ok := findClosingRun(b, contentStart, openLen)
		if !ok {
			i = contentStart
			continue
		}

		for k := contentStart; k < closeStart; k++ {
			b[k] = ' '
		}
		i = closeEnd
	}
	return string(b)
}

// backtickRunEnd returns the index just past the run of backticks starting at i.
func backtickRunEnd(b []byte, i int) int {
	for i < len(b) && b[i] == '`' {
		i++
	}
	return i
}

// findClosingRun locates the next backtick run of exactly width starting at or
// after pos, returning its bounds. ok is false when no matching run exists.
func findClosingRun(b []byte, pos, width int) (start, end int, ok bool) {
	for j := pos; j < len(b); {
		if b[j] != '`' {
			j++
			continue
		}
		runStart := j
		j = backtickRunEnd(b, j)
		if j-runStart == width {
			return runStart, j, true
		}
	}
	return 0, 0, false
}

func normalizeLinkContext(lines []string) string {
	if len(lines) == 0 {
		return ""
	}
	return normalizePreviewLine(strings.Join(lines, " "))
}

func rewriteLinkLines(lines []textLine, oldKey, newName string, respectFences bool) (rewritten string, changes int) {
	var (
		b          strings.Builder
		inFence    bool
		fenceRune  rune
		fenceWidth int
	)

	for _, line := range lines {
		if respectFences {
			trimmed := strings.TrimSpace(trimLine(line.text))
			if marker, width, ok := fenceStart(trimmed); ok {
				if inFence && marker == fenceRune && width >= fenceWidth {
					inFence = false
				} else if !inFence {
					inFence = true
					fenceRune = marker
					fenceWidth = width
				}
				b.WriteString(line.text)
				continue
			}
			if inFence {
				b.WriteString(line.text)
				continue
			}
		}

		updated := line.text
		var lineChanges int
		updated, lineChanges = rewriteWikiLinks(updated, oldKey, newName)
		changes += lineChanges
		updated, lineChanges = rewriteMarkdownLinks(updated, oldKey, newName)
		changes += lineChanges
		b.WriteString(updated)
	}

	return b.String(), changes
}

func rewriteWikiLinks(line, oldKey, newName string) (rewritten string, changes int) {
	matches := wikiLinkRE.FindAllStringIndex(maskInlineCode(line), -1)
	if len(matches) == 0 {
		return line, 0
	}

	var (
		b    strings.Builder
		last int
	)
	for _, match := range matches {
		start, end := match[0], match[1]
		b.WriteString(line[last:start])

		full := line[start:end]
		inner := full[2 : len(full)-2]
		targetPart, label, hasLabel, escapedSeparator := splitWikiLinkParts(inner)
		base, suffix := splitTargetSuffix(strings.TrimSpace(targetPart))
		if documentKey(base) != oldKey {
			b.WriteString(full)
			last = end
			continue
		}

		newTarget := newName + suffix
		if strings.HasSuffix(strings.ToLower(strings.TrimSpace(base)), ".md") {
			newTarget = newName + ".md" + suffix
		}
		if hasLabel {
			separator := "|"
			if escapedSeparator {
				separator = `\|`
			}
			b.WriteString("[[" + newTarget + separator + label + "]]")
		} else {
			b.WriteString("[[" + newTarget + "]]")
		}
		changes++
		last = end
	}
	b.WriteString(line[last:])

	return b.String(), changes
}

func rewriteMarkdownLinks(line, oldKey, newName string) (rewritten string, changes int) {
	matches := markdownLinkRE.FindAllStringSubmatchIndex(maskInlineCode(line), -1)
	if len(matches) == 0 {
		return line, 0
	}

	var (
		b    strings.Builder
		last int
	)
	for _, match := range matches {
		start, end := match[0], match[1]
		destStart, destEnd := match[2], match[3]
		full := line[start:end]

		b.WriteString(line[last:start])
		if strings.HasPrefix(full, "!") {
			b.WriteString(full)
			last = end
			continue
		}

		newDest, changed := rewriteMarkdownDestination(line[destStart:destEnd], oldKey, newName)
		if !changed {
			b.WriteString(full)
			last = end
			continue
		}

		b.WriteString(line[start:destStart])
		b.WriteString(newDest)
		b.WriteString(line[destEnd:end])
		changes++
		last = end
	}
	b.WriteString(line[last:])

	return b.String(), changes
}

func rewriteMarkdownDestination(dest, oldKey, newName string) (string, bool) {
	pathStart, pathEnd, ok := destinationBounds(dest)
	if !ok {
		return dest, false
	}

	rawPath := dest[pathStart:pathEnd]
	if documentKey(rawPath) != oldKey {
		return dest, false
	}

	newPath := replacePathBase(rawPath, newName)
	return dest[:pathStart] + newPath + dest[pathEnd:], true
}

func parseWikiLinks(line string) []Link {
	matches := wikiLinkRE.FindAllString(line, -1)
	links := make([]Link, 0, len(matches))
	for _, match := range matches {
		inner := match[2 : len(match)-2]
		targetPart, _, _, _ := splitWikiLinkParts(inner)
		targetPart = strings.TrimSpace(targetPart)
		base, suffix := splitTargetSuffix(targetPart)
		key := documentKey(base)
		if key == "" {
			continue
		}
		links = append(links, Link{
			Kind:          LinkWiki,
			DisplayTarget: displayTarget(base, suffix),
			TargetKey:     key,
			RawTarget:     rawLinkTarget(base),
		})
	}
	return links
}

func splitWikiLinkParts(inner string) (targetPart, label string, hasLabel, escapedSeparator bool) {
	for i := 0; i < len(inner); i++ {
		if inner[i] != '|' {
			continue
		}
		if hasEscapedPipe(inner, i) {
			return inner[:i-1], inner[i+1:], true, true
		}
		return inner[:i], inner[i+1:], true, false
	}
	return inner, "", false, false
}

func hasEscapedPipe(value string, pipeIndex int) bool {
	if pipeIndex == 0 || value[pipeIndex-1] != '\\' {
		return false
	}

	slashes := 0
	for i := pipeIndex - 1; i >= 0 && value[i] == '\\'; i-- {
		slashes++
	}
	return slashes%2 == 1
}

func parseMarkdownLinks(line string) []Link {
	matches := markdownLinkRE.FindAllStringSubmatch(line, -1)
	var links []Link
	for _, match := range matches {
		if strings.HasPrefix(match[0], "!") {
			continue
		}
		target, ok := parseMarkdownTarget(match[1])
		if !ok {
			continue
		}
		base, suffix := splitTargetSuffix(target)
		key := documentKey(base)
		if key == "" {
			continue
		}
		links = append(links, Link{
			Kind:          LinkMarkdown,
			DisplayTarget: displayTarget(base, suffix),
			TargetKey:     key,
			RawTarget:     rawLinkTarget(base),
		})
	}
	return links
}

func parseMarkdownTarget(dest string) (string, bool) {
	start, end, ok := destinationBounds(dest)
	if !ok {
		return "", false
	}
	target := strings.TrimSpace(dest[start:end])
	if target == "" {
		return "", false
	}
	if strings.HasPrefix(target, "#") || strings.Contains(target, "://") || strings.HasPrefix(target, "mailto:") {
		return "", false
	}
	return target, true
}

func destinationBounds(dest string) (start, end int, ok bool) {
	start = 0
	for start < len(dest) && (dest[start] == ' ' || dest[start] == '\t') {
		start++
	}
	if start >= len(dest) {
		return 0, 0, false
	}

	if dest[start] == '<' {
		end = strings.IndexByte(dest[start:], '>')
		if end == -1 {
			return 0, 0, false
		}
		return start + 1, start + end, true
	}

	end = start
	for end < len(dest) && dest[end] != ' ' && dest[end] != '\t' {
		end++
	}
	return start, end, end > start
}

func replacePathBase(rawPath, newName string) string {
	base, suffix := splitTargetSuffix(rawPath)
	dir, file := path.Split(base)
	ext := path.Ext(file)
	replacement := newName
	if strings.EqualFold(ext, ".md") {
		replacement += ".md"
	}
	return dir + replacement + suffix
}

func splitTargetSuffix(target string) (base, suffix string) {
	idx := strings.Index(target, "#")
	if idx == -1 {
		return target, ""
	}
	return target[:idx], target[idx:]
}

func displayTarget(base, suffix string) string {
	name := normalizeDocumentName(base)
	if name == "" {
		name = strings.TrimSpace(base)
	}
	return name + suffix
}

func normalizeDocumentName(value string) string {
	value = strings.TrimSpace(strings.Trim(value, "<>"))
	if value == "" {
		return ""
	}

	base, _ := splitTargetSuffix(value)
	base = strings.TrimPrefix(base, "./")
	base = path.Clean(base)
	base = path.Base(base)
	if base == "." || base == "/" {
		return ""
	}

	ext := path.Ext(base)
	if strings.EqualFold(ext, ".md") {
		base = strings.TrimSuffix(base, ext)
	}
	return norm.NFC.String(strings.TrimSpace(base))
}

func documentKey(value string) string {
	name := normalizeDocumentName(value)
	if name == "" {
		return ""
	}
	return strings.ToLower(name)
}

// rawLinkTarget normalizes a link target into the literal repo-relative-ish
// path used for recursive resolution: <> trimmed, fragment removed, surrounding
// whitespace trimmed. The directory portion is preserved (unlike documentKey).
func rawLinkTarget(base string) string {
	base = strings.TrimSpace(strings.Trim(strings.TrimSpace(base), "<>"))
	base, _ = splitTargetSuffix(base)
	return strings.TrimSpace(base)
}

// documentPathKey computes the canonical key for a repo-relative path: cleaned,
// slash-separated, final ".md" stripped, NFC-normalized, lowercased. In a flat
// vault (a path with no directory component) this equals documentKey, so flat
// behavior is the single-directory special case of the path rule.
func documentPathKey(relPath string) string {
	cleaned := cleanRelPath(relPath)
	if cleaned == "" {
		return ""
	}
	if ext := path.Ext(cleaned); strings.EqualFold(ext, ".md") {
		cleaned = strings.TrimSuffix(cleaned, ext)
	}
	return strings.ToLower(norm.NFC.String(cleaned))
}

// cleanRelPath normalizes a slash path: trims, drops a leading "./", and
// path.Clean-s it. Returns "" for empty or "." results.
func cleanRelPath(p string) string {
	p = strings.TrimSpace(p)
	if p == "" {
		return ""
	}
	p = strings.TrimPrefix(p, "./")
	p = path.Clean(p)
	if p == "." || p == "/" {
		return ""
	}
	return p
}

// resolveTargetRel joins a link's raw target against the source document's
// directory and cleans the result, yielding a repo-relative path. An empty
// sourceDir (top-level document) makes this a plain clean of the target.
func resolveTargetRel(sourceDir, rawTarget string) string {
	rawTarget = strings.TrimSpace(rawTarget)
	if sourceDir == "" || sourceDir == "." {
		return cleanRelPath(rawTarget)
	}
	return cleanRelPath(path.Join(sourceDir, rawTarget))
}

// relPathFromDir returns the path of target as written from sourceDir, using
// "../" hops as needed. Both arguments are slash paths relative to the vault
// root. Used to recompute markdown link text after a rename moves a file.
func relPathFromDir(sourceDir, target string) string {
	sourceDir = cleanRelPath(sourceDir)
	target = cleanRelPath(target)
	if sourceDir == "" {
		return target
	}

	srcParts := strings.Split(sourceDir, "/")
	tgtParts := strings.Split(target, "/")
	i := 0
	for i < len(srcParts) && i < len(tgtParts) && strings.EqualFold(srcParts[i], tgtParts[i]) {
		i++
	}

	var b strings.Builder
	for j := i; j < len(srcParts); j++ {
		b.WriteString("../")
	}
	b.WriteString(strings.Join(tgtParts[i:], "/"))
	rel := b.String()
	if rel == "" {
		return "."
	}
	return rel
}

// lastSegment returns the final path component of a slash path.
func lastSegment(relPath string) string {
	relPath = cleanRelPath(relPath)
	if idx := strings.LastIndex(relPath, "/"); idx >= 0 {
		return relPath[idx+1:]
	}
	return relPath
}

// dirSegment returns the directory portion of a slash path, or "" for a
// top-level path.
func dirSegment(relPath string) string {
	relPath = cleanRelPath(relPath)
	if idx := strings.LastIndex(relPath, "/"); idx >= 0 {
		return relPath[:idx]
	}
	return ""
}

func fenceStart(line string) (marker rune, width int, ok bool) {
	if len(line) < 3 {
		return 0, 0, false
	}
	if strings.HasPrefix(line, "```") {
		return '`', countPrefix(line, '`'), true
	}
	if strings.HasPrefix(line, "~~~") {
		return '~', countPrefix(line, '~'), true
	}
	return 0, 0, false
}

func countPrefix(line string, marker rune) int {
	count := 0
	for _, r := range line {
		if r != marker {
			break
		}
		count++
	}
	return count
}
