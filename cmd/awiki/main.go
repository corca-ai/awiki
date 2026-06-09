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

// errLintIssues is a sentinel returned by lintCmd when the report has issues.
// The report itself is already printed to stdout, so main only needs the
// non-zero exit status without an additional "awiki: ..." message.
var errLintIssues = errors.New("lint issues found")

type wantedOptions struct {
	root      string
	recursive bool
	limit     int
	sources   int
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
		if !errors.Is(err, errLintIssues) {
			printCommandError(err)
		}
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

// newFlagSet constructs a FlagSet wired to stderr with a standard Usage
// callback. The supplied lines are printed verbatim, followed by a blank
// line, "Flags:", and the auto-generated flag defaults.
func newFlagSet(name string, usage []string) *flag.FlagSet {
	fs := flag.NewFlagSet(name, flag.ContinueOnError)
	fs.SetOutput(stderr)
	fs.Usage = func() {
		for _, line := range usage {
			errln(line)
		}
		errln("")
		errln("Flags:")
		fs.PrintDefaults()
	}
	return fs
}

// parseFlags wraps fs.Parse so callers can early-return on -h/--help without
// surfacing flag.ErrHelp as a runtime error. Returns helped=true when the
// help message was shown; in that case err is nil and the caller should
// return immediately.
func parseFlags(fs *flag.FlagSet, args []string) (helped bool, err error) {
	err = fs.Parse(args)
	if errors.Is(err, flag.ErrHelp) {
		return true, nil
	}
	return false, err
}

// addCommonFlags registers the flags shared by every command: -root and the
// opt-in -recursive (-r) flag. Recursion is never automatic.
func addCommonFlags(fs *flag.FlagSet) (root *string, recursive *bool) {
	root = fs.String("root", ".", "Path to wiki root")
	rec := new(bool)
	fs.BoolVar(rec, "recursive", false, "Recurse into subdirectories, identifying documents by repo-relative path")
	fs.BoolVar(rec, "r", false, "Shorthand for -recursive")
	return root, rec
}

func loadVaultFor(root string, recursive bool) (*wiki.Vault, error) {
	return wiki.LoadWithOptions(root, wiki.Options{Recursive: recursive})
}

func lintCmd(args []string) error {
	fs := newFlagSet("lint", []string{
		"Usage: awiki lint [flags]",
		"",
		"Fail if the wiki contains orphan documents or disconnected islands.",
		"Also prints largest_component_ratio, orphan_rate, and content_coverage.",
	})
	root, recursive := addCommonFlags(fs)
	if helped, err := parseFlags(fs, args); helped || err != nil {
		return err
	}

	vault, err := loadVaultFor(*root, *recursive)
	if err != nil {
		return err
	}

	printer := newOutputFormatter()
	report := vault.Lint()
	if report.HasIssues() {
		printer.printLine(formatLintReport(vault, report))
		return errLintIssues
	}

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
	fs := newFlagSet("path", []string{
		"Usage: awiki path [flags] <from> <to>",
		"",
		"Print the shortest undirected path between two documents.",
		"Quote document names that contain spaces.",
		"",
		"Example:",
		"  awiki path \"The China study (book)\" \"What to Eat\"",
	})
	root, recursive := addCommonFlags(fs)
	if helped, err := parseFlags(fs, args); helped || err != nil {
		return err
	}

	rest := fs.Args()
	if len(rest) != 2 {
		fs.Usage()
		return errors.New("path requires exactly two document arguments")
	}

	vault, err := loadVaultFor(*root, *recursive)
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
	fs := newFlagSet("avg-shortest-path", []string{
		"Usage: awiki avg-shortest-path [flags]",
		"",
		"Estimate the average shortest path length on the largest connected component",
		"and print sampled longer-than-average paths.",
	})
	root, recursive := addCommonFlags(fs)
	samples := fs.Int("samples", 500, "Number of sampled document pairs")
	examples := fs.Int("examples", 1, "Number of sampled longer-than-average paths to print")
	seed := fs.Int64("seed", 1, "Random seed for pair sampling")
	if helped, err := parseFlags(fs, args); helped || err != nil {
		return err
	}
	if len(fs.Args()) != 0 {
		fs.Usage()
		return errors.New("avg-shortest-path does not accept positional arguments")
	}

	vault, err := loadVaultFor(*root, *recursive)
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
	fs := newFlagSet("rename", []string{
		"Usage: awiki rename [flags] <old> <new>",
		"",
		"Rename a document and rewrite links that point to it.",
		"Quote document names that contain spaces.",
		"",
		"Example:",
		"  awiki rename \"Old Note\" \"New Note\"",
	})
	root, recursive := addCommonFlags(fs)
	if helped, err := parseFlags(fs, args); helped || err != nil {
		return err
	}

	rest := fs.Args()
	if len(rest) != 2 {
		fs.Usage()
		return errors.New("rename requires exactly two document arguments")
	}

	vault, err := loadVaultFor(*root, *recursive)
	if err != nil {
		return err
	}

	result, err := vault.Rename(rest[0], rest[1])
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
	fs := newFlagSet("links", []string{
		"Usage: awiki links [flags] <document>",
		"",
		"Show inbound and outbound links for a document.",
		"Quote document names that contain spaces.",
		"",
		"Example:",
		"  awiki links \"Books Ive read\"",
	})
	root, recursive := addCommonFlags(fs)
	if helped, err := parseFlags(fs, args); helped || err != nil {
		return err
	}

	rest := fs.Args()
	if len(rest) != 1 {
		fs.Usage()
		return errors.New("links requires exactly one document argument")
	}

	vault, err := loadVaultFor(*root, *recursive)
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

	vault, err := loadVaultFor(options.root, options.recursive)
	if err != nil {
		return err
	}

	printWantedPages(newOutputFormatter(), limitWantedPages(vault.AllWantedPages(), options.limit), options.sources)
	return nil
}

func parseWantedOptions(args []string) (wantedOptions, error) {
	fs := newFlagSet("wanted", []string{
		"Usage: awiki wanted [flags]",
		"",
		"Show the most-linked missing pages and the lines that reference them.",
		"",
		"Example:",
		"  awiki wanted -n 10",
	})
	root, recursive := addCommonFlags(fs)
	limit := fs.Int("n", 10, "Number of missing pages to print")
	sources := fs.Int("sources", wantedSourcePreviewLimit, "Maximum referencing lines to show per missing page")
	helped, err := parseFlags(fs, args)
	if err != nil {
		return wantedOptions{}, err
	}
	if helped {
		return wantedOptions{}, flag.ErrHelp
	}
	if len(fs.Args()) != 0 {
		fs.Usage()
		return wantedOptions{}, errors.New("wanted does not accept positional arguments")
	}
	if *limit < 0 {
		return wantedOptions{}, errors.New("wanted limit must not be negative")
	}
	if *sources < 0 {
		return wantedOptions{}, errors.New("wanted sources must not be negative")
	}

	return wantedOptions{
		root:      *root,
		recursive: *recursive,
		limit:     *limit,
		sources:   *sources,
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

func printWantedPages(printer outputFormatter, pages []wiki.WantedPage, sourceLimit int) {
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
		if sourceLimit > 0 && len(sources) > sourceLimit {
			sources = sources[:sourceLimit]
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
	if limit <= 3 {
		return strings.Repeat(".", limit)
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
	errf("awiki: %s\n", err.Error())
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
