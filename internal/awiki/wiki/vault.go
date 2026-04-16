package wiki

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type Document struct {
	Name        string
	Key         string
	Path        string
	Excerpt     string
	FrontMatter FrontMatter
	Links       []Link
}

type LinkSummary struct {
	Name    string
	Missing bool
}

type LintReport struct {
	DocumentCount        int
	LargestComponentSize int
	CoveredDocuments     int
	Orphans              []string
	Islands              [][]string
}

func (r LintReport) HasIssues() bool {
	return len(r.Orphans) > 0 || len(r.Islands) > 0
}

func (r LintReport) LargestComponentRatio() float64 {
	return ratio(r.LargestComponentSize, r.DocumentCount)
}

func (r LintReport) OrphanRate() float64 {
	return ratio(len(r.Orphans), r.DocumentCount)
}

func (r LintReport) ContentCoverage() float64 {
	return ratio(r.CoveredDocuments, r.DocumentCount)
}

type Vault struct {
	Root        string
	Documents   []*Document
	docsByKey   map[string]*Document
	identifiers map[string][]*Document
	directed    map[string]map[string]struct{}
	inbound     map[string]map[string]struct{}
	undirected  map[string]map[string]struct{}
}

type cacheState struct {
	cache    vaultCache
	ok       bool
	dirty    bool
	next     vaultCache
	useCache bool
}

func Load(root string) (*Vault, error) {
	vault, _, err := loadVault(root, true)
	return vault, err
}

func loadVault(root string, useCache bool) (*Vault, loadStats, error) {
	absRoot, err := filepath.Abs(root)
	if err != nil {
		return nil, loadStats{}, err
	}

	entries, err := readSortedDirEntries(absRoot)
	if err != nil {
		return nil, loadStats{}, err
	}

	vault := newVault(absRoot)
	cache := newCacheState(absRoot, useCache)
	var stats loadStats

	for _, entry := range entries {
		cached, loaded, err := loadVaultEntry(vault, entry, cache)
		if err != nil {
			return nil, loadStats{}, err
		}
		if !loaded {
			continue
		}
		if cached {
			stats.CachedDocs++
		} else {
			stats.ParsedDocs++
			cache.dirty = true
		}
	}

	finalizeCacheState(absRoot, cache)

	vault.buildIdentifiers()
	vault.buildGraph()
	return vault, stats, nil
}

func readSortedDirEntries(root string) ([]os.DirEntry, error) {
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil, err
	}

	sort.Slice(entries, func(i, j int) bool {
		return strings.ToLower(entries[i].Name()) < strings.ToLower(entries[j].Name())
	})
	return entries, nil
}

func newVault(root string) *Vault {
	return &Vault{
		Root:        root,
		docsByKey:   make(map[string]*Document),
		identifiers: make(map[string][]*Document),
		directed:    make(map[string]map[string]struct{}),
		inbound:     make(map[string]map[string]struct{}),
		undirected:  make(map[string]map[string]struct{}),
	}
}

func newCacheState(root string, useCache bool) *cacheState {
	state := &cacheState{
		useCache: useCache,
		next: vaultCache{
			Version: vaultCacheVersion,
			Root:    root,
			Docs:    make(map[string]cachedDocument),
		},
	}
	if !useCache {
		return state
	}

	state.cache, state.ok = readVaultCache(root)
	if !state.ok {
		state.dirty = true
	}
	return state
}

func finalizeCacheState(root string, state *cacheState) {
	if !state.useCache {
		return
	}
	if !state.dirty && (!state.ok || len(state.cache.Docs) != len(state.next.Docs)) {
		state.dirty = true
	}
	if state.dirty {
		writeVaultCache(root, state.next)
	}
}

func loadVaultEntry(vault *Vault, entry os.DirEntry, state *cacheState) (cached, loaded bool, err error) {
	if entry.IsDir() || !strings.EqualFold(filepath.Ext(entry.Name()), ".md") {
		return false, false, nil
	}

	info, err := entry.Info()
	if err != nil {
		return false, false, err
	}

	filename := entry.Name()
	path := filepath.Join(vault.Root, filename)
	name := strings.TrimSuffix(filename, filepath.Ext(filename))
	key, err := validateDocumentKey(vault, filename, name)
	if err != nil {
		return false, false, err
	}

	doc, cached, err := loadDocument(path, filename, name, key, info, state.cache)
	if err != nil {
		return false, false, err
	}

	addDocumentToVault(vault, doc)
	state.next.Docs[filename] = cachedDocumentFor(doc, filename, info)
	return cached, true, nil
}

func validateDocumentKey(vault *Vault, filename, name string) (string, error) {
	key := documentKey(name)
	if key == "" {
		return "", fmt.Errorf("invalid document name %q", filename)
	}
	if existing, ok := vault.docsByKey[key]; ok {
		return "", fmt.Errorf("duplicate document names %q and %q", existing.Name, name)
	}
	return key, nil
}

func addDocumentToVault(vault *Vault, doc *Document) {
	vault.Documents = append(vault.Documents, doc)
	vault.docsByKey[doc.Key] = doc
}

func cachedDocumentFor(doc *Document, filename string, info os.FileInfo) cachedDocument {
	return cachedDocument{
		Filename:    filename,
		Name:        doc.Name,
		Key:         doc.Key,
		MTimeNS:     info.ModTime().UnixNano(),
		Size:        info.Size(),
		Excerpt:     doc.Excerpt,
		FrontMatter: doc.FrontMatter,
		Links:       cloneLinks(doc.Links),
	}
}

func loadDocument(path, filename, name, key string, info os.FileInfo, cache vaultCache) (*Document, bool, error) {
	if cached, ok := cache.Docs[filename]; ok &&
		cached.Name == name &&
		cached.Key == key &&
		cached.MTimeNS == info.ModTime().UnixNano() &&
		cached.Size == info.Size() {
		return &Document{
			Name:        cached.Name,
			Key:         cached.Key,
			Path:        path,
			Excerpt:     cached.Excerpt,
			FrontMatter: cached.FrontMatter,
			Links:       cloneLinks(cached.Links),
		}, true, nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false, err
	}
	content := string(data)
	return &Document{
		Name:        name,
		Key:         key,
		Path:        path,
		Excerpt:     FirstPreviewLine(content),
		FrontMatter: ParseFrontMatter(content),
		Links:       ParseLinks(content),
	}, false, nil
}

func (v *Vault) ResolveDocument(identifier string) (*Document, error) {
	key := documentKey(identifier)
	if key == "" {
		return nil, fmt.Errorf("document %q not found", identifier)
	}
	if doc, ok := v.docsByKey[key]; ok {
		return doc, nil
	}

	docs := uniqueDocuments(v.identifiers[key])
	switch len(docs) {
	case 0:
		return nil, fmt.Errorf("document %q not found", identifier)
	case 1:
		return docs[0], nil
	default:
		names := make([]string, 0, len(docs))
		for _, doc := range docs {
			names = append(names, doc.Name)
		}
		sort.Strings(names)
		return nil, fmt.Errorf("document identifier %q is ambiguous: %s", identifier, strings.Join(names, ", "))
	}
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
			seen["doc:"+documentKey(link.Resolved)] = LinkSummary{Name: link.Resolved}
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
	fromKey := documentKey(from)
	toKey := documentKey(to)
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
	fromKey := documentKey(from)
	toKey := documentKey(to)
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

func (v *Vault) Lint() LintReport {
	report := LintReport{
		DocumentCount: len(v.Documents),
	}
	visited := make(map[string]bool, len(v.Documents))
	var components [][]string

	for _, doc := range v.Documents {
		if strings.TrimSpace(doc.Excerpt) != "" {
			report.CoveredDocuments++
		}

		neighbors := v.undirected[doc.Key]
		if len(neighbors) == 0 {
			report.Orphans = append(report.Orphans, doc.Name)
			visited[doc.Key] = true
			continue
		}
		if visited[doc.Key] {
			continue
		}

		component := v.collectComponent(doc.Key, visited)
		sortNames(component)
		components = append(components, component)
	}

	sortNames(report.Orphans)
	sort.Slice(components, func(i, j int) bool {
		if len(components[i]) != len(components[j]) {
			return len(components[i]) > len(components[j])
		}
		return strings.ToLower(components[i][0]) < strings.ToLower(components[j][0])
	})
	if len(components) > 1 {
		report.Islands = components[1:]
	}
	if len(components) > 0 {
		report.LargestComponentSize = len(components[0])
	} else if len(report.Orphans) > 0 {
		report.LargestComponentSize = 1
	}

	return report
}

func (v *Vault) buildIdentifiers() {
	for _, doc := range v.Documents {
		v.addIdentifier(doc.Name, doc)
		if doc.FrontMatter.Title != "" {
			v.addIdentifier(doc.FrontMatter.Title, doc)
		}
		for _, alias := range doc.FrontMatter.Aliases {
			v.addIdentifier(alias, doc)
		}
	}
}

func (v *Vault) buildGraph() {
	for _, doc := range v.Documents {
		v.directed[doc.Key] = make(map[string]struct{})
		v.inbound[doc.Key] = make(map[string]struct{})
		v.undirected[doc.Key] = make(map[string]struct{})
	}

	for _, doc := range v.Documents {
		for i := range doc.Links {
			target, ok := v.docsByKey[doc.Links[i].TargetKey]
			if !ok {
				continue
			}

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

func (v *Vault) addIdentifier(identifier string, doc *Document) {
	key := documentKey(identifier)
	if key == "" {
		return
	}
	v.identifiers[key] = append(v.identifiers[key], doc)
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

func uniqueDocuments(docs []*Document) []*Document {
	if len(docs) <= 1 {
		return docs
	}

	seen := make(map[string]*Document, len(docs))
	for _, doc := range docs {
		seen[doc.Key] = doc
	}

	unique := make([]*Document, 0, len(seen))
	for _, doc := range seen {
		unique = append(unique, doc)
	}
	sort.Slice(unique, func(i, j int) bool {
		return strings.ToLower(unique[i].Name) < strings.ToLower(unique[j].Name)
	})
	return unique
}

func sortNames(names []string) {
	sort.Slice(names, func(i, j int) bool {
		return strings.ToLower(names[i]) < strings.ToLower(names[j])
	})
}

func ratio(count, total int) float64 {
	if total == 0 {
		return 0
	}
	return float64(count) / float64(total)
}
