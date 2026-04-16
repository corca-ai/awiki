package wiki

import (
	"sort"
	"strings"
)

type WantedSource struct {
	Document string
	Context  string
	Mentions int
}

type WantedPage struct {
	Name            string
	Mentions        int
	SourceDocuments int
	Sources         []WantedSource
}

type wantedAggregate struct {
	Name        string
	Mentions    int
	documents   map[string]struct{}
	sourceIndex map[string]int
	Sources     []WantedSource
}

func (v *Vault) WantedPages(limit int) []WantedPage {
	pages := v.AllWantedPages()
	if limit <= 0 {
		return nil
	}
	if len(pages) > limit {
		pages = pages[:limit]
	}
	return pages
}

func (v *Vault) AllWantedPages() []WantedPage {
	return v.allWantedPages()
}

func (v *Vault) allWantedPages() []WantedPage {
	aggregates := make(map[string]*wantedAggregate)
	for _, doc := range v.Documents {
		for _, link := range doc.Links {
			v.addWantedLink(aggregates, doc, link)
		}
	}

	pages := buildWantedPages(aggregates)
	sortWantedPages(pages)
	return pages
}

func (v *Vault) addWantedLink(aggregates map[string]*wantedAggregate, doc *Document, link Link) {
	name, ok := wantedName(link)
	if !ok {
		return
	}

	agg := ensureWantedAggregate(aggregates, link.TargetKey, name)
	agg.Mentions++
	agg.documents[doc.Key] = struct{}{}
	addWantedSource(agg, doc, wantedContext(doc, link))
}

func wantedName(link Link) (string, bool) {
	if link.Resolved != "" || link.TargetKey == "" {
		return "", false
	}

	name := normalizeDocumentName(link.DisplayTarget)
	if name == "" {
		name = strings.TrimSpace(link.DisplayTarget)
	}
	if name == "" {
		return "", false
	}
	return name, true
}

func ensureWantedAggregate(aggregates map[string]*wantedAggregate, targetKey, name string) *wantedAggregate {
	agg, ok := aggregates[targetKey]
	if ok {
		return agg
	}

	agg = &wantedAggregate{
		Name:        name,
		documents:   make(map[string]struct{}),
		sourceIndex: make(map[string]int),
	}
	aggregates[targetKey] = agg
	return agg
}

func wantedContext(doc *Document, link Link) string {
	context := strings.TrimSpace(link.Context)
	if context != "" {
		return context
	}

	context = strings.TrimSpace(doc.Excerpt)
	if context != "" {
		return context
	}
	return "(empty)"
}

func addWantedSource(agg *wantedAggregate, doc *Document, context string) {
	indexKey := doc.Key + "\x00" + context
	if idx, ok := agg.sourceIndex[indexKey]; ok {
		agg.Sources[idx].Mentions++
		return
	}

	agg.sourceIndex[indexKey] = len(agg.Sources)
	agg.Sources = append(agg.Sources, WantedSource{
		Document: doc.Name,
		Context:  context,
		Mentions: 1,
	})
}

func buildWantedPages(aggregates map[string]*wantedAggregate) []WantedPage {
	pages := make([]WantedPage, 0, len(aggregates))
	for _, agg := range aggregates {
		pages = append(pages, WantedPage{
			Name:            agg.Name,
			Mentions:        agg.Mentions,
			SourceDocuments: len(agg.documents),
			Sources:         sortedWantedSources(agg.Sources),
		})
	}
	return pages
}

func sortedWantedSources(sources []WantedSource) []WantedSource {
	sorted := make([]WantedSource, len(sources))
	copy(sorted, sources)
	sort.Slice(sorted, func(i, j int) bool {
		if sorted[i].Mentions != sorted[j].Mentions {
			return sorted[i].Mentions > sorted[j].Mentions
		}
		if !strings.EqualFold(sorted[i].Document, sorted[j].Document) {
			return strings.ToLower(sorted[i].Document) < strings.ToLower(sorted[j].Document)
		}
		return strings.ToLower(sorted[i].Context) < strings.ToLower(sorted[j].Context)
	})
	return sorted
}

func sortWantedPages(pages []WantedPage) {
	sort.Slice(pages, func(i, j int) bool {
		if pages[i].Mentions != pages[j].Mentions {
			return pages[i].Mentions > pages[j].Mentions
		}
		if pages[i].SourceDocuments != pages[j].SourceDocuments {
			return pages[i].SourceDocuments > pages[j].SourceDocuments
		}
		return strings.ToLower(pages[i].Name) < strings.ToLower(pages[j].Name)
	})
}
