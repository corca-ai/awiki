package wiki

import (
	"crypto/sha256"
	"encoding/gob"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
)

const (
	vaultCacheVersion = 7
	vaultCacheDir     = "v7"
)

var userCacheDir = os.UserCacheDir

type loadStats struct {
	CachedDocs int
	ParsedDocs int
}

type vaultCache struct {
	Version   int
	Root      string
	Recursive bool
	Docs      map[string]cachedDocument
}

type cachedDocument struct {
	RelFile     string
	Name        string
	Key         string
	RelPath     string
	MTimeNS     int64
	Size        int64
	Excerpt     string
	FrontMatter FrontMatter
	Links       []Link
	LinkOnly    []LinkOnlyLine
}

func init() {
	gob.Register(vaultCache{})
	gob.Register(cachedDocument{})
	gob.Register(FrontMatter{})
	gob.Register(Link{})
	gob.Register(LinkOnlyLine{})
}

func cacheFilePath(root string, recursive bool) (string, error) {
	base, err := userCacheDir()
	if err != nil {
		return "", err
	}

	seed := filepath.Clean(root)
	if recursive {
		seed += "\x00recursive=1"
	}
	sum := sha256.Sum256([]byte(seed))
	hash := hex.EncodeToString(sum[:])
	return filepath.Join(base, "awiki", vaultCacheDir, "roots", hash, "vault.gob"), nil
}

func readVaultCache(root string, recursive bool) (vaultCache, bool) {
	path, err := cacheFilePath(root, recursive)
	if err != nil {
		return vaultCache{}, false
	}

	file, err := os.Open(path)
	if err != nil {
		return vaultCache{}, false
	}
	defer func() {
		_ = file.Close()
	}()

	var cache vaultCache
	if err := gob.NewDecoder(file).Decode(&cache); err != nil {
		return vaultCache{}, false
	}
	if cache.Version != vaultCacheVersion || cache.Root != root || cache.Recursive != recursive {
		return vaultCache{}, false
	}
	if cache.Docs == nil {
		cache.Docs = make(map[string]cachedDocument)
	}
	return cache, true
}

func writeVaultCache(root string, cache vaultCache) {
	path, err := cacheFilePath(root, cache.Recursive)
	if err != nil {
		return
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return
	}

	tmp, err := os.CreateTemp(filepath.Dir(path), ".vault-*.gob")
	if err != nil {
		return
	}
	tmpPath := tmp.Name()
	defer func() {
		_ = os.Remove(tmpPath)
	}()

	if err := gob.NewEncoder(tmp).Encode(cache); err != nil {
		_ = tmp.Close()
		return
	}
	if err := tmp.Close(); err != nil {
		return
	}
	_ = os.Rename(tmpPath, path)
	pruneStaleCacheDirs()
}

// pruneStaleCacheDirs best-effort removes cache directories from previous
// schema versions (e.g. "v5") so upgrades do not accumulate dead cache trees.
func pruneStaleCacheDirs() {
	base, err := userCacheDir()
	if err != nil {
		return
	}
	for v := 1; v < vaultCacheVersion; v++ {
		_ = os.RemoveAll(filepath.Join(base, "awiki", fmt.Sprintf("v%d", v)))
	}
}

func cloneLinks(links []Link) []Link {
	if len(links) == 0 {
		return nil
	}

	cloned := make([]Link, len(links))
	copy(cloned, links)
	for i := range cloned {
		cloned[i].Resolved = ""
	}
	return cloned
}

func cloneLinkOnlyLines(lines []LinkOnlyLine) []LinkOnlyLine {
	if len(lines) == 0 {
		return nil
	}

	cloned := make([]LinkOnlyLine, len(lines))
	copy(cloned, lines)
	return cloned
}
