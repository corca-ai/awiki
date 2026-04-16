package wiki

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func BenchmarkLoadWithoutCache(b *testing.B) {
	useTempCacheDir(b)

	root := benchmarkWiki(b, 10000)
	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		if _, _, err := loadVault(root, false); err != nil {
			b.Fatalf("loadVault() error = %v", err)
		}
	}
}

func BenchmarkLoadWarmCache(b *testing.B) {
	useTempCacheDir(b)

	root := benchmarkWiki(b, 10000)
	if _, _, err := loadVault(root, true); err != nil {
		b.Fatalf("initial loadVault() error = %v", err)
	}

	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		if _, _, err := loadVault(root, true); err != nil {
			b.Fatalf("loadVault() error = %v", err)
		}
	}
}

func benchmarkWiki(b testing.TB, docs int) string {
	b.Helper()

	root := b.TempDir()
	for i := 0; i < docs; i++ {
		name := fmt.Sprintf("Doc %05d", i)
		next := fmt.Sprintf("Doc %05d", (i+1)%docs)
		prev := fmt.Sprintf("Doc %05d", (i+docs-1)%docs)
		content := fmt.Sprintf("[[%s]]\n[[%s]]\n", prev, next)
		writeFileForBench(b, filepath.Join(root, name+".md"), content)
	}
	return root
}

func writeFileForBench(b testing.TB, path, content string) {
	b.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		b.Fatalf("write %s: %v", path, err)
	}
}
