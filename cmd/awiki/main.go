package main

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/corca-ai/awiki/internal/awiki/wiki"
)

var version = "dev"

var (
	stdout io.Writer = os.Stdout
	stderr io.Writer = os.Stderr
)

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(2)
	}

	var err error
	switch os.Args[1] {
	case "help", "--help", "-help", "-h":
		usage()
	case "lint":
		err = lintCmd(os.Args[2:])
	case "avg-shortest-path":
		err = avgShortestPathCmd(os.Args[2:])
	case "path":
		err = pathCmd(os.Args[2:])
	case "rename":
		err = renameCmd(os.Args[2:])
	case "links":
		err = linksCmd(os.Args[2:])
	case "version", "--version", "-version":
		outln(version)
	default:
		unknownCmd(os.Args[1:])
	}

	if err != nil {
		printCommandError(err)
		os.Exit(1)
	}
}

func usage() {
	errln("Usage: awiki <command> [args]")
	errln("")
	errln("Commands:")
	errln("  awiki help")
	errln("      Show this help")
	errln("  awiki lint [flags]")
	errln("      Validate the wiki graph")
	errln("  awiki avg-shortest-path [flags]")
	errln("      Estimate average shortest path length")
	errln("  awiki path [flags] <from> <to>")
	errln("      Print the shortest path between two documents")
	errln("  awiki rename [flags] <old> <new>")
	errln("      Rename a document and update links to it")
	errln("  awiki links [flags] <document>")
	errln("      Show inbound and outbound links for a document")
	errln("  awiki version")
	errln("      Print build version")
	errln("")
	errln("Examples:")
	errln("  awiki path \"The China study (book)\" \"What to Eat\"")
	errln("  awiki rename \"Old Note\" \"New Note\"")
	errln("  awiki links \"Books Ive read\"")
	errln("")
	errln("Use `awiki <command> -h` for command-specific help.")
}

func unknownCmd(args []string) {
	for _, arg := range args {
		if arg == "--version" || arg == "-version" {
			outln(version)
			return
		}
	}

	errf("awiki: unknown command %q\n\n", args[0])
	usage()
	os.Exit(2)
}

func hasHelpFlag(args []string) bool {
	for _, arg := range args {
		if arg == "-help" || arg == "--help" || arg == "-h" {
			return true
		}
	}
	return false
}

func lintCmd(args []string) error {
	fs := flag.NewFlagSet("lint", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki lint [flags]")
		errln("")
		errln("Fail if the wiki contains orphan documents or disconnected islands.")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	vault, err := wiki.Load(*root)
	if err != nil {
		return err
	}

	report := vault.Lint()
	if report.HasIssues() {
		return errors.New(formatLintReport(vault, report))
	}

	printer := newOutputFormatter()
	printer.printComment(fmt.Sprintf("ok connected_graph documents=%d", len(vault.Documents)))
	return nil
}

func pathCmd(args []string) error {
	fs := flag.NewFlagSet("path", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki path [flags] <from> <to>")
		errln("")
		errln("Print the shortest undirected path between two documents.")
		errln("Quote document names that contain spaces.")
		errln("")
		errln("Example:")
		errln("  awiki path \"The China study (book)\" \"What to Eat\"")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	rest := fs.Args()
	if len(rest) != 2 {
		fs.Usage()
		return errors.New("path requires exactly two document arguments")
	}

	vault, err := wiki.Load(*root)
	if err != nil {
		return err
	}

	from, err := vault.ResolveDocument(rest[0])
	if err != nil {
		return err
	}
	to, err := vault.ResolveDocument(rest[1])
	if err != nil {
		return err
	}

	path, err := vault.ShortestPath(from.Name, to.Name)
	if err != nil {
		return err
	}

	printPathLines(newOutputFormatter(), vault, path)
	return nil
}

func avgShortestPathCmd(args []string) error {
	fs := flag.NewFlagSet("avg-shortest-path", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki avg-shortest-path [flags]")
		errln("")
		errln("Estimate the average shortest path length on the largest connected component.")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	samples := fs.Int("samples", 500, "Number of sampled document pairs")
	examples := fs.Int("examples", 1, "Number of sampled longer-than-average paths to print")
	seed := fs.Int64("seed", 1, "Random seed for pair sampling")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}
	if len(fs.Args()) != 0 {
		fs.Usage()
		return errors.New("avg-shortest-path does not accept positional arguments")
	}

	vault, err := wiki.Load(*root)
	if err != nil {
		return err
	}

	report, err := vault.ApproximateAverageShortestPath(*samples, *examples, *seed)
	if err != nil {
		return err
	}

	printer := newOutputFormatter()
	printer.printLine(fmt.Sprintf(
		"// largest_component_size=%d samples=%d average_shortest_path=%.4f",
		report.ComponentSize,
		report.SampleCount,
		report.Average,
	))
	if len(report.LongerPaths) == 0 {
		return nil
	}

	for i, sample := range report.LongerPaths {
		if i > 0 {
			printer.printBlankLine()
		}
		printPathLines(printer, vault, sample.Nodes)
	}
	return nil
}

func renameCmd(args []string) error {
	fs := flag.NewFlagSet("rename", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki rename [flags] <old> <new>")
		errln("")
		errln("Rename a document and rewrite links that point to it.")
		errln("Quote document names that contain spaces.")
		errln("")
		errln("Example:")
		errln("  awiki rename \"Old Note\" \"New Note\"")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	rest := fs.Args()
	if len(rest) != 2 {
		fs.Usage()
		return errors.New("rename requires exactly two document arguments")
	}

	result, err := wiki.Rename(*root, rest[0], rest[1])
	if err != nil {
		return err
	}

	printer := newOutputFormatter()
	printer.printComment(fmt.Sprintf("rename old=%s.md new=%s.md", result.OldName, result.NewName))
	printer.printComment(fmt.Sprintf(
		"links_updated=%d files_touched=%d title_updated=%t",
		result.LinksUpdated,
		result.FilesTouched,
		result.TitleUpdated,
	))
	return nil
}

func linksCmd(args []string) error {
	fs := flag.NewFlagSet("links", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki links [flags] <document>")
		errln("")
		errln("Show inbound and outbound links for a document.")
		errln("Quote document names that contain spaces.")
		errln("")
		errln("Example:")
		errln("  awiki links \"Books Ive read\"")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	rest := fs.Args()
	if len(rest) != 1 {
		fs.Usage()
		return errors.New("links requires exactly one document argument")
	}

	vault, err := wiki.Load(*root)
	if err != nil {
		return err
	}

	doc, err := vault.ResolveDocument(rest[0])
	if err != nil {
		return err
	}

	printer := newOutputFormatter()
	printer.printComment("this page")
	printDocumentLine(printer, doc)
	printer.printComment("incoming links")
	printNameLines(printer, vault, vault.InboundNames(doc.Name))
	printer.printComment("outgoing links")
	printLinkSummaryLines(printer, vault, vault.OutboundSummaries(doc.Name))
	return nil
}

func truncateRunes(value string, limit int) string {
	if limit <= 0 {
		return ""
	}

	runes := []rune(value)
	if len(runes) <= limit {
		return value
	}
	return strings.TrimSpace(string(runes[:limit-3])) + "..."
}

func formatLintReport(vault *wiki.Vault, report wiki.LintReport) string {
	var b strings.Builder
	b.WriteString(fmt.Sprintf("// lint_failed orphans=%d islands=%d", len(report.Orphans), len(report.Islands)))

	if len(report.Orphans) > 0 {
		b.WriteString("\n// orphan")
		for _, name := range report.Orphans {
			b.WriteString("\n")
			b.WriteString(formatDocumentIssue(vault, name))
		}
	}

	if len(report.Islands) > 0 {
		for i, island := range report.Islands {
			b.WriteString(fmt.Sprintf("\n// island=%d", i+1))
			for _, name := range island {
				b.WriteString("\n")
				b.WriteString(formatDocumentIssue(vault, name))
			}
		}
	}

	return b.String()
}

func formatDocumentIssue(vault *wiki.Vault, name string) string {
	doc, err := vault.ResolveDocument(name)
	if err != nil {
		return formatDocumentLine(name, "")
	}

	preview := documentPreview(doc)
	return formatDocumentLine(doc.Name, preview)
}

func printCommandError(err error) {
	message := err.Error()
	if strings.HasPrefix(message, "// ") {
		errln(message)
		return
	}
	errf("awiki: %s\n", message)
}

func outf(format string, args ...any) {
	_, _ = fmt.Fprintf(stdout, format, args...)
}

func outln(args ...any) {
	_, _ = fmt.Fprintln(stdout, args...)
}

func errf(format string, args ...any) {
	_, _ = fmt.Fprintf(stderr, format, args...)
}

func errln(args ...any) {
	_, _ = fmt.Fprintln(stderr, args...)
}
