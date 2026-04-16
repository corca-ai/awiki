package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/corca-ai/awiki/internal/awiki/wiki"
)

const previewLimit = 140

const (
	ansiReset  = "\x1b[0m"
	ansiBold   = "\x1b[1m"
	ansiCyan   = "\x1b[36m"
	ansiYellow = "\x1b[33m"
	ansiGreen  = "\x1b[32m"
)

type outputFormatter struct {
	color bool
}

func newOutputFormatter() outputFormatter {
	return outputFormatter{color: stdoutSupportsColor()}
}

func stdoutSupportsColor() bool {
	if os.Getenv("NO_COLOR") != "" || os.Getenv("CLICOLOR") == "0" {
		return false
	}
	term := os.Getenv("TERM")
	if term == "" || term == "dumb" {
		return false
	}

	file, ok := stdout.(*os.File)
	if !ok || file != os.Stdout {
		return false
	}

	info, err := file.Stat()
	if err != nil {
		return false
	}
	return info.Mode()&os.ModeCharDevice != 0
}

func (f outputFormatter) printLine(value string) {
	_, _ = fmt.Fprintln(stdout, value)
}

func (f outputFormatter) printBlankLine() {
	_, _ = fmt.Fprintln(stdout)
}

func (f outputFormatter) printComment(value string) {
	f.printLine(f.comment(value))
}

func (f outputFormatter) comment(value string) string {
	if !f.color {
		return "// " + value
	}
	return ansiGreen + ansiBold + "// " + value + ansiReset
}

func (f outputFormatter) wikiLink(name string, missing bool) string {
	label := "[[" + name + "]]"
	if !f.color {
		return label
	}

	color := ansiCyan
	if missing {
		color = ansiYellow
	}
	return color + ansiBold + label + ansiReset
}

func (f outputFormatter) documentLine(name, preview string) string {
	return formatDocumentLineWithLink(f.wikiLink(name, false), preview)
}

func (f outputFormatter) missingLine(name string) string {
	return formatDocumentLineWithLink(f.wikiLink(name, true), "(missing)")
}

func printDocumentLine(printer outputFormatter, doc *wiki.Document) {
	printer.printLine(printer.documentLine(doc.Name, documentPreview(doc)))
}

func printMissingLine(printer outputFormatter, name string) {
	printer.printLine(printer.missingLine(name))
}

func printNameLines(printer outputFormatter, vault *wiki.Vault, names []string) {
	if len(names) == 0 {
		printer.printComment("none")
		return
	}

	for _, name := range names {
		doc, err := vault.ResolveDocument(name)
		if err != nil {
			printer.printLine(formatDocumentLine(name, ""))
			continue
		}
		printDocumentLine(printer, doc)
	}
}

func printLinkSummaryLines(printer outputFormatter, vault *wiki.Vault, links []wiki.LinkSummary) {
	if len(links) == 0 {
		printer.printComment("none")
		return
	}

	for _, link := range links {
		if link.Missing {
			printMissingLine(printer, link.Name)
			continue
		}

		doc, err := vault.ResolveDocument(link.Name)
		if err != nil {
			printer.printLine(formatDocumentLine(link.Name, ""))
			continue
		}
		printDocumentLine(printer, doc)
	}
}

func printPathLines(printer outputFormatter, vault *wiki.Vault, names []string) {
	printNameLines(printer, vault, names)
}

func documentPreview(doc *wiki.Document) string {
	return truncateRunes(strings.TrimSpace(doc.Excerpt), previewLimit)
}

func formatDocumentLine(name, preview string) string {
	return formatDocumentLineWithLink("[["+name+"]]", preview)
}

func formatDocumentLineWithLink(link, preview string) string {
	preview = strings.TrimSpace(preview)
	if preview == "" {
		return link + ": (empty)"
	}
	return link + ": " + preview
}
