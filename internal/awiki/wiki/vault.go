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

func (v *Vault) addIdentifier(identifier string, doc *Document) {
	key := documentKey(identifier)
	if key == "" {
		return
	}
	v.identifiers[key] = append(v.identifiers[key], doc)
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
