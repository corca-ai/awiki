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
	Resolved      string
}

var (
	wikiLinkRE     = regexp.MustCompile(`\[\[[^\]\n]+\]\]`)
	markdownLinkRE = regexp.MustCompile(`!?\[[^\]\n]*\]\(([^)\n]+)\)`)
)

func ParseLinks(content string) []Link {
	fm := ParseFrontMatter(content)
	if !fm.Present {
		return parseLinkLines(scanLines(content), true)
	}

	links := parseLinkLines(scanLines(content[:fm.BodyOffset]), false)
	links = append(links, parseLinkLines(scanLines(content[fm.BodyOffset:]), true)...)
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

func parseLinkLines(lines []textLine, respectFences bool) []Link {
	var (
		links      []Link
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
				continue
			}
			if inFence {
				continue
			}
		}

		links = append(links, parseWikiLinks(line.text)...)
		links = append(links, parseMarkdownLinks(line.text)...)
	}

	return links
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
	matches := wikiLinkRE.FindAllStringIndex(line, -1)
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
	matches := markdownLinkRE.FindAllStringSubmatchIndex(line, -1)
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
