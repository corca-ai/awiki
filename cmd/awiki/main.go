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

const wantedSourcePreviewLimit = 10

type wantedOptions struct {
	root  string
	limit int
}

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
	case "wanted":
		err = wantedCmd(os.Args[2:])
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
	errln("awiki helps maintain the quality of a flat-file Markdown wiki.")
	errln("Quality here means keeping notes well connected, reducing orphans and disconnected islands, keeping most pages inside one large component,")
	errln("avoiding empty stubs, and making long paths and heavily linked missing pages easy to inspect.")
	errln("")
	errln("Commands:")
	errln("  awiki lint [flags]")
	errln("      Validate the wiki graph")
	errln("  awiki avg-shortest-path [flags]")
	errln("      Estimate average shortest path length and print sampled long paths")
	errln("  awiki path [flags] <from> <to>")
	errln("      Print the shortest path between two documents")
	errln("  awiki rename [flags] <old> <new>")
	errln("      Rename a document and update links to it")
	errln("  awiki links [flags] <document>")
	errln("      Show inbound and outbound links for a document")
	errln("  awiki wanted [flags]")
	errln("      Show the most-linked missing pages")
	errln("")
	errln("Examples:")
	errln("  awiki path \"The China study (book)\" \"What to Eat\"")
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
		errln("Also prints largest_component_ratio, orphan_rate, and content_coverage.")
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
	printer.printComment(fmt.Sprintf(
		"ok connected_graph documents=%d largest_component_ratio=%.4f orphan_rate=%.4f content_coverage=%.4f",
		report.DocumentCount,
		report.LargestComponentRatio(),
		report.OrphanRate(),
		report.ContentCoverage(),
	))
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
		errln("Estimate the average shortest path length on the largest connected component")
		errln("and print sampled longer-than-average paths.")
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

func wantedCmd(args []string) error {
	options, err := parseWantedOptions(args)
	if err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	vault, err := wiki.Load(options.root)
	if err != nil {
		return err
	}

	printWantedPages(newOutputFormatter(), limitWantedPages(vault.AllWantedPages(), options.limit))
	return nil
}

func parseWantedOptions(args []string) (wantedOptions, error) {
	fs := flag.NewFlagSet("wanted", flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		errln("Usage: awiki wanted [flags]")
		errln("")
		errln("Show the most-linked missing pages and the lines that reference them.")
		errln("")
		errln("Example:")
		errln("  awiki wanted -n 10")
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}

	root := fs.String("root", ".", "Path to wiki root")
	limit := fs.Int("n", 10, "Number of missing pages to print")
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return wantedOptions{}, flag.ErrHelp
		}
		return wantedOptions{}, err
	}
	if len(fs.Args()) != 0 {
		fs.Usage()
		return wantedOptions{}, errors.New("wanted does not accept positional arguments")
	}
	if *limit < 0 {
		return wantedOptions{}, errors.New("wanted limit must not be negative")
	}

	return wantedOptions{
		root:  *root,
		limit: *limit,
	}, nil
}

func limitWantedPages(pages []wiki.WantedPage, limit int) []wiki.WantedPage {
	if limit <= 0 {
		return nil
	}
	if len(pages) > limit {
		return pages[:limit]
	}
	return pages
}

func printWantedPages(printer outputFormatter, pages []wiki.WantedPage) {
	if len(pages) == 0 {
		printer.printLine("_ none")
		return
	}

	for i, page := range pages {
		if i > 0 {
			printer.printBlankLine()
		}
		printer.printLine(printer.wantedHeader(page.Name, page.Mentions))
		printer.printBlankLine()

		sources := page.Sources
		if len(sources) > wantedSourcePreviewLimit {
			sources = sources[:wantedSourcePreviewLimit]
		}
		for _, source := range sources {
			printer.printLine(printer.wantedSourceLine(source.Document, source.Context))
		}
		if len(page.Sources) > len(sources) {
			printer.printLine("_ ...")
		}
	}
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
	fmt.Fprintf(&b,
		"// lint_failed documents=%d orphans=%d islands=%d largest_component_ratio=%.4f orphan_rate=%.4f content_coverage=%.4f",
		report.DocumentCount,
		len(report.Orphans),
		len(report.Islands),
		report.LargestComponentRatio(),
		report.OrphanRate(),
		report.ContentCoverage(),
	)

	if len(report.Orphans) > 0 {
		b.WriteString("\n// orphan")
		for _, name := range report.Orphans {
			b.WriteString("\n")
			b.WriteString(formatDocumentIssue(vault, name))
		}
	}

	if len(report.Islands) > 0 {
		for i, island := range report.Islands {
			fmt.Fprintf(&b, "\n// island=%d", i+1)
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

func outln(args ...any) {
	_, _ = fmt.Fprintln(stdout, args...)
}

func errf(format string, args ...any) {
	_, _ = fmt.Fprintf(stderr, format, args...)
}

func errln(args ...any) {
	_, _ = fmt.Fprintln(stderr, args...)
}
