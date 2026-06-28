package wiki

import (
	"sort"
	"strings"
	"unicode"
)

type LinkOnlyLine struct {
	Line int
	Text string
}

type linkSpan struct {
	start int
	end   int
}

func FindLinkOnlyLines(content string) []LinkOnlyLine {
	lines := scanLines(content)
	fm := ParseFrontMatter(content)
	bodyOffset := 0
	if fm.Present {
		bodyOffset = fm.BodyOffset
	}

	var (
		issues     []LinkOnlyLine
		inFence    bool
		fenceRune  rune
		fenceWidth int
	)
	for i, line := range lines {
		if line.end <= bodyOffset {
			continue
		}

		trimmed := strings.TrimSpace(trimLine(line.text))
		if marker, width, ok := fenceStart(trimmed); ok {
			inFence, fenceRune, fenceWidth = nextFenceState(inFence, fenceRune, fenceWidth, marker, width)
			continue
		}
		if inFence || trimmed == "" {
			continue
		}
		if !isLinkOnlyLine(trimmed) {
			continue
		}

		issues = append(issues, LinkOnlyLine{
			Line: i + 1,
			Text: trimmed,
		})
	}
	return issues
}

func isLinkOnlyLine(line string) bool {
	spans := documentLinkSpans(line)
	if len(spans) != 1 {
		return false
	}

	span := spans[0]
	remainder := line[:span.start] + strings.Repeat(" ", span.end-span.start) + line[span.end:]
	remainder = stripLineOnlyMarkdown(remainder)
	return !containsLetterOrDigit(remainder)
}

func documentLinkSpans(line string) []linkSpan {
	masked := maskInlineCode(line)
	var spans []linkSpan

	for _, match := range wikiLinkRE.FindAllStringIndex(masked, -1) {
		inner := masked[match[0]+2 : match[1]-2]
		targetPart, _, _, _ := splitWikiLinkParts(inner)
		base, _ := splitTargetSuffix(strings.TrimSpace(targetPart))
		if documentKey(base) == "" {
			continue
		}
		spans = append(spans, linkSpan{start: match[0], end: match[1]})
	}

	for _, match := range markdownLinkRE.FindAllStringSubmatchIndex(masked, -1) {
		if strings.HasPrefix(masked[match[0]:match[1]], "!") {
			continue
		}
		target, ok := parseMarkdownTarget(masked[match[2]:match[3]])
		if !ok {
			continue
		}
		base, _ := splitTargetSuffix(target)
		if documentKey(base) == "" {
			continue
		}
		spans = append(spans, linkSpan{start: match[0], end: match[1]})
	}

	sort.Slice(spans, func(i, j int) bool {
		return spans[i].start < spans[j].start
	})
	return spans
}

func stripLineOnlyMarkdown(value string) string {
	for {
		next := strings.TrimSpace(value)
		next = stripBlockPrefix(next)
		next = stripListPrefix(next)
		next = stripTaskPrefix(next)
		next = stripHeadingPrefix(next)
		next = strings.TrimSpace(next)
		if next == strings.TrimSpace(value) {
			return next
		}
		value = next
	}
}

func stripBlockPrefix(value string) string {
	if strings.HasPrefix(value, ">") {
		return value[1:]
	}
	return value
}

func stripListPrefix(value string) string {
	if len(value) >= 2 && strings.ContainsRune("-*+", rune(value[0])) && isSpace(value[1]) {
		return value[2:]
	}

	i := 0
	for i < len(value) && value[i] >= '0' && value[i] <= '9' {
		i++
	}
	if i == 0 || i+1 >= len(value) {
		return value
	}
	if (value[i] == '.' || value[i] == ')') && isSpace(value[i+1]) {
		return value[i+2:]
	}
	return value
}

func stripTaskPrefix(value string) string {
	if len(value) < 4 || value[0] != '[' || value[2] != ']' || !isSpace(value[3]) {
		return value
	}
	switch value[1] {
	case ' ', 'x', 'X':
		return value[4:]
	default:
		return value
	}
}

func stripHeadingPrefix(value string) string {
	count := 0
	for count < len(value) && value[count] == '#' {
		count++
	}
	if count == 0 || count > 6 {
		return value
	}
	if count == len(value) || isSpace(value[count]) {
		return value[count:]
	}
	return value
}

func containsLetterOrDigit(value string) bool {
	for _, r := range value {
		if unicode.IsLetter(r) || unicode.IsDigit(r) {
			return true
		}
	}
	return false
}

func isSpace(b byte) bool {
	return b == ' ' || b == '\t'
}
