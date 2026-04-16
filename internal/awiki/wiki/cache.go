package wiki

import (
	"crypto/sha256"
	"encoding/gob"
	"encoding/hex"
	"os"
	"path/filepath"
)

const (
	vaultCacheVersion = 3
	vaultCacheDir     = "v3"
)

var userCacheDir = os.UserCacheDir

type loadStats struct {
	CachedDocs int
	ParsedDocs int
}

type vaultCache struct {
	Version int
	Root    string
	Docs    map[string]cachedDocument
}

type cachedDocument struct {
	Filename    string
	Name        string
	Key         string
	MTimeNS     int64
	Size        int64
	Excerpt     string
	FrontMatter FrontMatter
	Links       []Link
}

func init() {
	gob.Register(vaultCache{})
	gob.Register(cachedDocument{})
	gob.Register(FrontMatter{})
	gob.Register(Link{})
}

func cacheFilePath(root string) (string, error) {
	base, err := userCacheDir()
	if err != nil {
		return "", err
	}

	sum := sha256.Sum256([]byte(filepath.Clean(root)))
	hash := hex.EncodeToString(sum[:])
	return filepath.Join(base, "awiki", vaultCacheDir, "roots", hash, "vault.gob"), nil
}

func readVaultCache(root string) (vaultCache, bool) {
	path, err := cacheFilePath(root)
	if err != nil {
		return vaultCache{}, false
	}

	file, err := os.Open(path)
	if err != nil {
		return vaultCache{}, false
	}
	defer file.Close()

	var cache vaultCache
	if err := gob.NewDecoder(file).Decode(&cache); err != nil {
		return vaultCache{}, false
	}
	if cache.Version != vaultCacheVersion || cache.Root != root {
		return vaultCache{}, false
	}
	if cache.Docs == nil {
		cache.Docs = make(map[string]cachedDocument)
	}
	return cache, true
}

func writeVaultCache(root string, cache vaultCache) {
	path, err := cacheFilePath(root)
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
