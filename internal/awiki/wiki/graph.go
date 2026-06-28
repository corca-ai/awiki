package wiki

import (
	"fmt"
	"sort"
	"strings"
)

type LinkSummary struct {
	Name    string
	Missing bool
}

func (v *Vault) buildGraph() {
	for _, doc := range v.Documents {
		v.directed[doc.Key] = make(map[string]struct{})
		v.inbound[doc.Key] = make(map[string]struct{})
		v.undirected[doc.Key] = make(map[string]struct{})
	}

	for _, doc := range v.Documents {
		for i := range doc.Links {
			target, ok := v.resolveLinkTarget(doc, doc.Links[i])
			if !ok {
				continue
			}

			// Mutate the link in place so callers see the canonical document
			// name, not whatever spelling the source used. Resolution lives
			// here because it depends on the full docsByKey index, which only
			// exists after Load completes.
			doc.Links[i].Resolved = target.Name
			if doc.Key == target.Key {
				continue
			}
			if _, seen := v.directed[doc.Key][target.Key]; seen {
				continue
			}

			v.directed[doc.Key][target.Key] = struct{}{}
			v.inbound[target.Key][doc.Key] = struct{}{}
			v.undirected[doc.Key][target.Key] = struct{}{}
			v.undirected[target.Key][doc.Key] = struct{}{}
		}
	}
}

// resolveLinkTarget resolves a single link to a document. Flat vaults resolve
// by basename key (the original behavior). Recursive vaults resolve markdown
// links and path-qualified wikilinks by repo-relative path, and bare wikilinks
// by unique basename — matching Obsidian's getFirstLinkpathDest precedence.
func (v *Vault) resolveLinkTarget(doc *Document, link Link) (*Document, bool) {
	if !v.recursive {
		target, ok := v.docsByKey[link.TargetKey]
		return target, ok
	}
	return v.resolveRecursive(dirSegment(doc.RelPath), link.Kind, link.RawTarget, link.TargetKey)
}

func (v *Vault) resolveRecursive(sourceDir string, kind LinkKind, rawTarget, baseKey string) (*Document, bool) {
	if kind == LinkMarkdown {
		// Markdown link destinations are always source-relative.
		if doc, ok := v.docsByKey[documentPathKey(resolveTargetRel(sourceDir, rawTarget))]; ok {
			return doc, true
		}
		return nil, false
	}

	// Wikilink.
	if strings.HasPrefix(rawTarget, "./") || strings.HasPrefix(rawTarget, "../") {
		if doc, ok := v.docsByKey[documentPathKey(resolveTargetRel(sourceDir, rawTarget))]; ok {
			return doc, true
		}
		return nil, false
	}
	if strings.ContainsAny(rawTarget, "/\\") {
		// Path-qualified: vault-absolute first (highest precedence), then relative.
		if doc, ok := v.docsByKey[documentPathKey(cleanRelPath(rawTarget))]; ok {
			return doc, true
		}
		if doc, ok := v.docsByKey[documentPathKey(resolveTargetRel(sourceDir, rawTarget))]; ok {
			return doc, true
		}
		return nil, false
	}

	// Bare basename: resolve source-independently; on a collision the
	// root/shortest path wins (the deterministic basenames ordering).
	if docs := v.basenames[baseKey]; len(docs) > 0 {
		return docs[0], true
	}
	return nil, false
}

func (v *Vault) InboundNames(identifier string) []string {
	doc, err := v.ResolveDocument(identifier)
	if err != nil {
		return nil
	}

	names := make([]string, 0, len(v.inbound[doc.Key]))
	for key := range v.inbound[doc.Key] {
		names = append(names, v.docsByKey[key].Name)
	}
	sortNames(names)
	return names
}

func (v *Vault) OutboundSummaries(identifier string) []LinkSummary {
	doc, err := v.ResolveDocument(identifier)
	if err != nil {
		return nil
	}

	seen := make(map[string]LinkSummary)
	for _, link := range doc.Links {
		if link.Resolved != "" {
			seen["doc:"+strings.ToLower(link.Resolved)] = LinkSummary{Name: link.Resolved}
			continue
		}
		seen["missing:"+strings.ToLower(link.DisplayTarget)] = LinkSummary{
			Name:    link.DisplayTarget,
			Missing: true,
		}
	}

	summaries := make([]LinkSummary, 0, len(seen))
	for _, summary := range seen {
		summaries = append(summaries, summary)
	}
	sort.Slice(summaries, func(i, j int) bool {
		if summaries[i].Missing != summaries[j].Missing {
			return !summaries[i].Missing
		}
		return strings.ToLower(summaries[i].Name) < strings.ToLower(summaries[j].Name)
	})
	return summaries
}

func (v *Vault) EdgeDirection(from, to string) string {
	fromKey := v.documentKeyFor(from)
	toKey := v.documentKeyFor(to)
	_, forward := v.directed[fromKey][toKey]
	_, backward := v.directed[toKey][fromKey]

	switch {
	case forward && backward:
		return "<->"
	case forward:
		return "->"
	case backward:
		return "<-"
	default:
		return "--"
	}
}

func (v *Vault) ShortestPath(from, to string) ([]string, error) {
	fromKey := v.documentKeyFor(from)
	toKey := v.documentKeyFor(to)
	if fromKey == "" || toKey == "" {
		return nil, fmt.Errorf("path endpoints must not be empty")
	}
	return v.shortestPathBetweenKeys(fromKey, toKey)
}

func (v *Vault) shortestPathBetweenKeys(fromKey, toKey string) ([]string, error) {
	if fromKey == toKey {
		return []string{v.docsByKey[fromKey].Name}, nil
	}

	queue := []string{fromKey}
	prev := map[string]string{fromKey: ""}

	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]

		for _, next := range v.sortedNeighbors(current) {
			if _, seen := prev[next]; seen {
				continue
			}
			prev[next] = current
			if next == toKey {
				return v.rebuildPath(prev, toKey), nil
			}
			queue = append(queue, next)
		}
	}

	return nil, fmt.Errorf("no path between %q and %q", v.docsByKey[fromKey].Name, v.docsByKey[toKey].Name)
}

func (v *Vault) sortedNeighbors(key string) []string {
	neighbors := make([]string, 0, len(v.undirected[key]))
	for neighbor := range v.undirected[key] {
		neighbors = append(neighbors, neighbor)
	}
	sort.Slice(neighbors, func(i, j int) bool {
		return strings.ToLower(v.docsByKey[neighbors[i]].Name) < strings.ToLower(v.docsByKey[neighbors[j]].Name)
	})
	return neighbors
}

func (v *Vault) largestConnectedComponentKeys() []string {
	visited := make(map[string]bool, len(v.Documents))
	var best []string

	for _, doc := range v.Documents {
		if visited[doc.Key] {
			continue
		}
		if len(v.undirected[doc.Key]) == 0 {
			visited[doc.Key] = true
			continue
		}

		component := v.collectComponentKeys(doc.Key, visited)
		sort.Slice(component, func(i, j int) bool {
			return strings.ToLower(v.docsByKey[component[i]].Name) < strings.ToLower(v.docsByKey[component[j]].Name)
		})
		if len(component) > len(best) {
			best = component
			continue
		}
		if len(component) == len(best) && len(component) > 0 &&
			strings.ToLower(v.docsByKey[component[0]].Name) < strings.ToLower(v.docsByKey[best[0]].Name) {
			best = component
		}
	}

	return best
}

func (v *Vault) rebuildPath(prev map[string]string, target string) []string {
	var reversed []string
	for current := target; current != ""; current = prev[current] {
		reversed = append(reversed, v.docsByKey[current].Name)
	}

	for i, j := 0, len(reversed)-1; i < j; i, j = i+1, j-1 {
		reversed[i], reversed[j] = reversed[j], reversed[i]
	}
	return reversed
}

func (v *Vault) collectComponent(start string, visited map[string]bool) []string {
	queue := []string{start}
	visited[start] = true
	var component []string

	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]
		component = append(component, v.docsByKey[current].Name)

		for _, neighbor := range v.sortedNeighbors(current) {
			if visited[neighbor] {
				continue
			}
			visited[neighbor] = true
			queue = append(queue, neighbor)
		}
	}

	return component
}

func (v *Vault) collectComponentKeys(start string, visited map[string]bool) []string {
	queue := []string{start}
	visited[start] = true
	var component []string

	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]
		component = append(component, current)

		for _, neighbor := range v.sortedNeighbors(current) {
			if visited[neighbor] {
				continue
			}
			visited[neighbor] = true
			queue = append(queue, neighbor)
		}
	}

	return component
}
