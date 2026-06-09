package wiki

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type Document struct {
	Name string
	Key  string
	Path string
	// RelPath is the document's repo-relative slash path without the ".md"
	// suffix. In a flat vault it equals the basename; in a recursive vault it
	// carries directories (e.g. "goals/g1"). It is the canonical identity.
	RelPath     string
	Excerpt     string
	FrontMatter FrontMatter
	Links       []Link
}

// Options controls how a vault is loaded.
type Options struct {
	// Recursive walks subdirectories and identifies documents by repo-relative
	// path. When false (the default) only top-level *.md files are loaded and
	// documents are identified by basename, exactly as before.
	Recursive bool
}

type Vault struct {
	Root        string
	Documents   []*Document
	recursive   bool
	docsByKey   map[string]*Document
	identifiers map[string][]*Document
	// basenames maps a basename key to the documents carrying that basename,
	// sorted by repo-relative path. It drives Obsidian-style bare wikilink
	// resolution and excludes titles/aliases on purpose.
	basenames  map[string][]*Document
	directed   map[string]map[string]struct{}
	inbound    map[string]map[string]struct{}
	undirected map[string]map[string]struct{}
}

type cacheState struct {
	cache    vaultCache
	ok       bool
	dirty    bool
	next     vaultCache
	useCache bool
}

func Load(root string) (*Vault, error) {
	return LoadWithOptions(root, Options{})
}

// LoadWithOptions loads a vault, honoring Options. With Options{Recursive:true}
// it walks subdirectories and identifies documents by repo-relative path.
func LoadWithOptions(root string, opts Options) (*Vault, error) {
	vault, _, err := loadVault(root, opts, true)
	return vault, err
}

func loadVault(root string, opts Options, useCache bool) (*Vault, loadStats, error) {
	absRoot, err := filepath.Abs(root)
	if err != nil {
		return nil, loadStats{}, err
	}

	files, err := discoverFiles(absRoot, opts.Recursive)
	if err != nil {
		return nil, loadStats{}, err
	}

	vault := newVault(absRoot, opts.Recursive)
	cache := newCacheState(absRoot, opts.Recursive, useCache)
	var stats loadStats

	for _, file := range files {
		cached, loaded, err := loadVaultEntry(vault, file, cache)
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
	vault.buildBasenames()
	vault.buildGraph()
	return vault, stats, nil
}

// discoveredFile is a markdown document found during discovery, carrying both
// its absolute path and its repo-relative slash path (with the ".md" suffix).
type discoveredFile struct {
	abs     string
	relFile string
	info    os.FileInfo
}

func discoverFiles(root string, recursive bool) ([]discoveredFile, error) {
	if recursive {
		return discoverFilesRecursive(root)
	}
	return discoverFilesFlat(root)
}

func discoverFilesFlat(root string) ([]discoveredFile, error) {
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil, err
	}

	var files []discoveredFile
	for _, entry := range entries {
		if entry.IsDir() || !strings.EqualFold(filepath.Ext(entry.Name()), ".md") {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			return nil, err
		}
		files = append(files, discoveredFile{
			abs:     filepath.Join(root, entry.Name()),
			relFile: entry.Name(),
			info:    info,
		})
	}
	sortDiscoveredFiles(files)
	return files, nil
}

func discoverFilesRecursive(root string) ([]discoveredFile, error) {
	var files []discoveredFile
	err := filepath.WalkDir(root, func(p string, d os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if d.IsDir() {
			if p != root && isIgnoredDir(d.Name()) {
				return filepath.SkipDir
			}
			return nil
		}
		if !strings.EqualFold(filepath.Ext(d.Name()), ".md") {
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		files = append(files, discoveredFile{
			abs:     p,
			relFile: filepath.ToSlash(rel),
			info:    info,
		})
		return nil
	})
	if err != nil {
		return nil, err
	}
	sortDiscoveredFiles(files)
	return files, nil
}

// isIgnoredDir reports whether a directory is excluded from recursive
// discovery: any dot-directory plus a few conventional build/vendor trees, so
// awiki and specdown enumerate the same documents.
func isIgnoredDir(name string) bool {
	if strings.HasPrefix(name, ".") {
		return true
	}
	switch name {
	case "node_modules", "vendor":
		return true
	}
	return false
}

func sortDiscoveredFiles(files []discoveredFile) {
	sort.Slice(files, func(i, j int) bool {
		return strings.ToLower(files[i].relFile) < strings.ToLower(files[j].relFile)
	})
}

func newVault(root string, recursive bool) *Vault {
	return &Vault{
		Root:        root,
		recursive:   recursive,
		docsByKey:   make(map[string]*Document),
		identifiers: make(map[string][]*Document),
		basenames:   make(map[string][]*Document),
		directed:    make(map[string]map[string]struct{}),
		inbound:     make(map[string]map[string]struct{}),
		undirected:  make(map[string]map[string]struct{}),
	}
}

func newCacheState(root string, recursive, useCache bool) *cacheState {
	state := &cacheState{
		useCache: useCache,
		next: vaultCache{
			Version:   vaultCacheVersion,
			Root:      root,
			Recursive: recursive,
			Docs:      make(map[string]cachedDocument),
		},
	}
	if !useCache {
		return state
	}

	state.cache, state.ok = readVaultCache(root, recursive)
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

func loadVaultEntry(vault *Vault, file discoveredFile, state *cacheState) (cached, loaded bool, err error) {
	relPath := strings.TrimSuffix(file.relFile, filepath.Ext(file.relFile))
	name := relPath
	if !vault.recursive {
		name = lastSegment(relPath)
	}
	key, err := validateDocumentKey(vault, file.relFile, relPath)
	if err != nil {
		return false, false, err
	}

	doc, cached, err := loadDocument(file.abs, file.relFile, name, key, relPath, file.info, state.cache)
	if err != nil {
		return false, false, err
	}

	addDocumentToVault(vault, doc)
	state.next.Docs[file.relFile] = cachedDocumentFor(doc, file.relFile, file.info)
	return cached, true, nil
}

func (v *Vault) documentKeyFor(relPath string) string {
	if v.recursive {
		return documentPathKey(relPath)
	}
	return documentKey(relPath)
}

func validateDocumentKey(vault *Vault, relFile, relPath string) (string, error) {
	key := vault.documentKeyFor(relPath)
	if key == "" {
		return "", fmt.Errorf("invalid document name %q", relFile)
	}
	if existing, ok := vault.docsByKey[key]; ok {
		return "", fmt.Errorf("duplicate document names %q and %q", existing.RelPath, relPath)
	}
	return key, nil
}

func addDocumentToVault(vault *Vault, doc *Document) {
	vault.Documents = append(vault.Documents, doc)
	vault.docsByKey[doc.Key] = doc
}

func cachedDocumentFor(doc *Document, relFile string, info os.FileInfo) cachedDocument {
	return cachedDocument{
		RelFile:     relFile,
		Name:        doc.Name,
		Key:         doc.Key,
		RelPath:     doc.RelPath,
		MTimeNS:     info.ModTime().UnixNano(),
		Size:        info.Size(),
		Excerpt:     doc.Excerpt,
		FrontMatter: doc.FrontMatter,
		Links:       cloneLinks(doc.Links),
	}
}

func loadDocument(path, relFile, name, key, relPath string, info os.FileInfo, cache vaultCache) (*Document, bool, error) {
	if cached, ok := cache.Docs[relFile]; ok &&
		cached.Name == name &&
		cached.Key == key &&
		cached.RelPath == relPath &&
		cached.MTimeNS == info.ModTime().UnixNano() &&
		cached.Size == info.Size() {
		return &Document{
			Name:        cached.Name,
			Key:         cached.Key,
			Path:        path,
			RelPath:     cached.RelPath,
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
		RelPath:     relPath,
		Excerpt:     FirstPreviewLine(content),
		FrontMatter: ParseFrontMatter(content),
		Links:       ParseLinks(content),
	}, false, nil
}

func (v *Vault) ResolveDocument(identifier string) (*Document, error) {
	// Exact identity first: in a recursive vault a repo-relative path
	// addresses a specific document even when its basename is shared.
	if doc, ok := v.docsByKey[v.documentKeyFor(identifier)]; ok {
		return doc, nil
	}

	// Fall back to the basename / title / alias index.
	key := documentKey(identifier)
	if key == "" {
		return nil, fmt.Errorf("document %q not found", identifier)
	}
	if !v.recursive {
		if doc, ok := v.docsByKey[key]; ok {
			return doc, nil
		}
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

// buildBasenames indexes documents by basename key, sorted by repo-relative
// path, for Obsidian-style bare wikilink resolution.
func (v *Vault) buildBasenames() {
	for _, doc := range v.Documents {
		key := documentKey(lastSegment(doc.RelPath))
		if key == "" {
			continue
		}
		v.basenames[key] = append(v.basenames[key], doc)
	}
	for key := range v.basenames {
		docs := v.basenames[key]
		sort.Slice(docs, func(i, j int) bool {
			// Obsidian's bare-link preference: the shortest path (closest to
			// the root) wins; ties broken deterministically by path.
			di := strings.Count(docs[i].RelPath, "/")
			dj := strings.Count(docs[j].RelPath, "/")
			if di != dj {
				return di < dj
			}
			return strings.ToLower(docs[i].RelPath) < strings.ToLower(docs[j].RelPath)
		})
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
