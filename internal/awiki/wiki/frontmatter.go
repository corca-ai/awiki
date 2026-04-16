package wiki

import "strings"

type FrontMatter struct {
	Present    bool
	BodyOffset int
	Title      string
	Aliases    []string
}

type textLine struct {
	text  string
	start int
	end   int
}

func ParseFrontMatter(content string) FrontMatter {
	lines := scanLines(content)
	endIndex, ok := frontMatterEndIndex(lines)
	if !ok {
		return FrontMatter{}
	}

	fm := FrontMatter{
		Present:    true,
		BodyOffset: lines[endIndex].end,
	}

	for i := 1; i < endIndex; i++ {
		line := trimLine(lines[i].text)
		if line == "" || startsIndented(line) {
			continue
		}

		key, value, ok := splitKeyValue(line)
		if !ok {
			continue
		}

		switch key {
		case "title":
			fm.Title = trimYAMLScalar(value)
		case "aliases":
			aliases, next := parseAliases(lines, i, endIndex, value)
			fm.Aliases = appendDedup(fm.Aliases, aliases...)
			i = next
		}
	}

	return fm
}

func UpdateFrontMatterTitle(content, oldTitle, newTitle string) (string, bool) {
	lines := scanLines(content)
	endIndex, ok := frontMatterEndIndex(lines)
	if !ok {
		return content, false
	}

	index, value, found := findTopLevelKey(lines, endIndex, "title")
	if !found || trimYAMLScalar(value) != oldTitle {
		return content, false
	}

	lines[index].text = replaceLineText(lines[index].text, "title: "+formatScalarLike(value, newTitle))
	return joinLines(lines), true
}

func parseAliases(lines []textLine, start, end int, value string) (aliases []string, next int) {
	trimmed := strings.TrimSpace(value)
	next = start
	if trimmed == "" {
		for i := start + 1; i < end; i++ {
			line := trimLine(lines[i].text)
			if line == "" {
				next = i
				continue
			}
			if !startsIndented(trimLinePreserveIndent(lines[i].text)) {
				break
			}

			item := strings.TrimSpace(line)
			if !strings.HasPrefix(item, "- ") {
				next = i
				continue
			}
			aliases = append(aliases, trimYAMLScalar(strings.TrimSpace(strings.TrimPrefix(item, "- "))))
			next = i
		}
		return aliases, next
	}

	if strings.HasPrefix(trimmed, "[") && strings.HasSuffix(trimmed, "]") {
		return parseInlineList(trimmed), next
	}

	return []string{trimYAMLScalar(trimmed)}, next
}

func parseInlineList(value string) []string {
	inner := strings.TrimSpace(value[1 : len(value)-1])
	if inner == "" {
		return nil
	}

	var (
		items   []string
		current strings.Builder
		quote   rune
	)

	for _, r := range inner {
		switch {
		case quote == 0 && (r == '\'' || r == '"'):
			quote = r
			current.WriteRune(r)
		case quote != 0 && r == quote:
			quote = 0
			current.WriteRune(r)
		case quote == 0 && r == ',':
			items = append(items, trimYAMLScalar(strings.TrimSpace(current.String())))
			current.Reset()
		default:
			current.WriteRune(r)
		}
	}

	if current.Len() > 0 {
		items = append(items, trimYAMLScalar(strings.TrimSpace(current.String())))
	}

	return items
}

func splitKeyValue(line string) (key, value string, ok bool) {
	idx := strings.Index(line, ":")
	if idx == -1 {
		return "", "", false
	}
	return strings.TrimSpace(line[:idx]), strings.TrimSpace(line[idx+1:]), true
}

func trimYAMLScalar(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 {
		if value[0] == '"' && value[len(value)-1] == '"' {
			return value[1 : len(value)-1]
		}
		if value[0] == '\'' && value[len(value)-1] == '\'' {
			return value[1 : len(value)-1]
		}
	}
	return value
}

func formatScalarLike(originalValue, replacement string) string {
	trimmed := strings.TrimSpace(originalValue)
	if len(trimmed) >= 2 {
		if trimmed[0] == '"' && trimmed[len(trimmed)-1] == '"' {
			return `"` + replacement + `"`
		}
		if trimmed[0] == '\'' && trimmed[len(trimmed)-1] == '\'' {
			return `'` + replacement + `'`
		}
	}
	return replacement
}

func scanLines(content string) []textLine {
	if content == "" {
		return nil
	}

	var lines []textLine
	start := 0
	for start < len(content) {
		end := start
		for end < len(content) && content[end] != '\n' {
			end++
		}
		if end < len(content) {
			end++
		}
		lines = append(lines, textLine{
			text:  content[start:end],
			start: start,
			end:   end,
		})
		start = end
	}

	return lines
}

func joinLines(lines []textLine) string {
	var b strings.Builder
	for _, line := range lines {
		b.WriteString(line.text)
	}
	return b.String()
}

func trimLine(line string) string {
	return strings.TrimRight(line, "\r\n")
}

func trimLinePreserveIndent(line string) string {
	return strings.TrimRight(line, "\r\n")
}

func frontMatterEndIndex(lines []textLine) (int, bool) {
	if len(lines) == 0 || trimLine(lines[0].text) != "---" {
		return -1, false
	}
	for i := 1; i < len(lines); i++ {
		if trimLine(lines[i].text) == "---" {
			return i, true
		}
	}
	return -1, false
}

func findTopLevelKey(lines []textLine, endIndex int, wanted string) (index int, value string, ok bool) {
	for i := 1; i < endIndex; i++ {
		line := trimLine(lines[i].text)
		if line == "" || startsIndented(line) {
			continue
		}
		key, value, ok := splitKeyValue(line)
		if ok && key == wanted {
			return i, value, true
		}
	}
	return -1, "", false
}

func replaceLineText(original, replacement string) string {
	switch {
	case strings.HasSuffix(original, "\r\n"):
		return replacement + "\r\n"
	case strings.HasSuffix(original, "\n"):
		return replacement + "\n"
	default:
		return replacement
	}
}

func startsIndented(line string) bool {
	return strings.HasPrefix(line, " ") || strings.HasPrefix(line, "\t")
}

func appendDedup(dst []string, values ...string) []string {
	seen := make(map[string]struct{}, len(dst))
	for _, value := range dst {
		seen[value] = struct{}{}
	}
	for _, value := range values {
		if value == "" {
			continue
		}
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		dst = append(dst, value)
	}
	return dst
}
