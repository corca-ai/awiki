package wiki

import "strings"

func FirstPreviewLine(content string) string {
	fm := ParseFrontMatter(content)
	body := content[fm.BodyOffset:]
	lines := scanLines(body)

	var (
		inFence    bool
		fenceRune  rune
		fenceWidth int
	)

	for _, line := range lines {
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
		if trimmed == "" {
			continue
		}
		if isSkippablePreviewLine(trimmed) {
			continue
		}
		return normalizePreviewLine(trimmed)
	}

	return ""
}

func normalizePreviewLine(line string) string {
	return strings.Join(strings.Fields(line), " ")
}

func isSkippablePreviewLine(line string) bool {
	return strings.HasPrefix(line, "#")
}
