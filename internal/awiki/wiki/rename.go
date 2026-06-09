package wiki

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
)

type RenameResult struct {
	OldName      string
	NewName      string
	FilesTouched int
	LinksUpdated int
	TitleUpdated bool
}

// renameTarget captures the resolved old document and its new identity.
type renameTarget struct {
	doc        *Document
	oldBase    string
	newBase    string
	newRelPath string
	newPath    string
}

// Rename renames a document inside the vault and rewrites links that point
// to it across every other document. Callers are expected to have just
// loaded the vault; the on-disk write is atomic per file. In a recursive
// vault the document keeps (or, given a path target, changes) its directory
// and relative link text is recomputed accordingly.
func (v *Vault) Rename(oldIdentifier, newIdentifier string) (RenameResult, error) {
	target, err := v.prepareRename(oldIdentifier, newIdentifier)
	if err != nil {
		return RenameResult{}, err
	}

	updated, touched, result, err := v.buildRenameUpdates(target)
	if err != nil {
		return RenameResult{}, err
	}
	if err := os.MkdirAll(filepath.Dir(target.newPath), 0o755); err != nil {
		return RenameResult{}, err
	}
	if err := os.Rename(target.doc.Path, target.newPath); err != nil {
		return RenameResult{}, err
	}

	moveUpdatedContent(updated, target.doc.Path, target.newPath)
	touched[target.newPath] = struct{}{}
	delete(touched, target.doc.Path)

	if err := writeUpdatedFiles(updated); err != nil {
		return RenameResult{}, err
	}

	result.FilesTouched = len(touched)
	if result.FilesTouched == 0 {
		result.FilesTouched = 1
	}
	return result, nil
}

func (v *Vault) prepareRename(oldIdentifier, newIdentifier string) (renameTarget, error) {
	doc, err := v.ResolveDocument(oldIdentifier)
	if err != nil {
		return renameTarget{}, err
	}

	ext := filepath.Ext(doc.Path)
	if !v.recursive {
		newName, err := validateRenameTarget(newIdentifier)
		if err != nil {
			return renameTarget{}, err
		}
		if existing, ok := v.docsByKey[documentKey(newName)]; ok && existing.Key != doc.Key {
			return renameTarget{}, fmt.Errorf("document %q already exists", existing.Name)
		}
		return renameTarget{
			doc:        doc,
			oldBase:    doc.Name,
			newBase:    newName,
			newRelPath: newName,
			newPath:    filepath.Join(v.Root, newName+ext),
		}, nil
	}

	newRelPath, err := validateRecursiveRenameTarget(newIdentifier, dirSegment(doc.RelPath))
	if err != nil {
		return renameTarget{}, err
	}
	if existing, ok := v.docsByKey[documentPathKey(newRelPath)]; ok && existing.Key != doc.Key {
		return renameTarget{}, fmt.Errorf("document %q already exists", existing.RelPath)
	}
	return renameTarget{
		doc:        doc,
		oldBase:    lastSegment(doc.RelPath),
		newBase:    lastSegment(newRelPath),
		newRelPath: newRelPath,
		newPath:    filepath.Join(v.Root, filepath.FromSlash(newRelPath)+ext),
	}, nil
}

func (v *Vault) buildRenameUpdates(target renameTarget) (updated map[string]string, touched map[string]struct{}, result RenameResult, err error) {
	updated = make(map[string]string)
	touched = make(map[string]struct{})
	result = RenameResult{
		OldName: renameDisplayName(v, target.doc),
		NewName: renameDisplayNewName(v, target),
	}

	newBaseUnique := v.newBaseIsUnique(target)
	for _, current := range v.Documents {
		rewritten, count, titleChanged, err := v.renameRewriteDocument(current, target, newBaseUnique)
		if err != nil {
			return nil, nil, RenameResult{}, err
		}
		result.LinksUpdated += count
		if titleChanged {
			result.TitleUpdated = true
		}
		if count == 0 && !titleChanged {
			continue
		}
		updated[current.Path] = rewritten
		touched[current.Path] = struct{}{}
	}

	return updated, touched, result, nil
}

// newBaseIsUnique reports whether the rename's new basename is unique across
// the vault (ignoring the document being renamed), so bare wikilinks can stay
// bare instead of being path-qualified. Always false in a flat vault.
func (v *Vault) newBaseIsUnique(target renameTarget) bool {
	if !v.recursive {
		return false
	}
	for _, d := range v.basenames[documentKey(target.newBase)] {
		if d != target.doc {
			return false
		}
	}
	return true
}

// renameRewriteDocument rewrites a single document's links (and, for the
// renamed document itself, its front-matter title) for a rename.
func (v *Vault) renameRewriteDocument(current *Document, target renameTarget, newBaseUnique bool) (rewritten string, count int, titleChanged bool, err error) {
	content, err := os.ReadFile(current.Path)
	if err != nil {
		return "", 0, false, err
	}

	if v.recursive {
		rewritten, count = v.rewriteDocumentLinksRecursive(
			string(content), dirSegment(current.RelPath), target, newBaseUnique)
	} else {
		rewritten, count = RewriteDocumentLinks(string(content), target.oldBase, target.newBase)
	}

	if current.Key == target.doc.Key {
		rewritten, titleChanged = UpdateFrontMatterTitle(rewritten, target.oldBase, target.newBase)
	}
	return rewritten, count, titleChanged, nil
}

// renameDisplayName / renameDisplayNewName produce the identity printed in the
// rename report: basename in a flat vault, repo-relative path in a recursive
// vault.
func renameDisplayName(v *Vault, doc *Document) string {
	if v.recursive {
		return doc.RelPath
	}
	return doc.Name
}

func renameDisplayNewName(v *Vault, target renameTarget) string {
	if v.recursive {
		return target.newRelPath
	}
	return target.newBase
}

func moveUpdatedContent(updated map[string]string, oldPath, newPath string) {
	if content, ok := updated[oldPath]; ok {
		delete(updated, oldPath)
		updated[newPath] = content
	}
}

func writeUpdatedFiles(updated map[string]string) error {
	for filePath, content := range updated {
		if err := writeFileAtomic(filePath, content, 0o644); err != nil {
			return err
		}
	}
	return nil
}

func validateRenameTarget(value string) (string, error) {
	name := normalizeDocumentName(value)
	if name == "" {
		return "", fmt.Errorf("new document name must not be empty")
	}
	if strings.ContainsAny(name, `/\<>:"|?*`) {
		return "", fmt.Errorf("new document name %q contains invalid path characters", name)
	}
	return name, nil
}

// validateRecursiveRenameTarget interprets a rename target in a recursive
// vault. A bare name keeps the document in its current directory; a target
// containing a slash is a repo-relative path. The result is a cleaned,
// slash-separated path without the ".md" suffix. Absolute paths and ".."
// segments that escape the vault root are rejected.
func validateRecursiveRenameTarget(value, sourceDir string) (string, error) {
	raw := strings.TrimSpace(strings.Trim(strings.TrimSpace(value), "<>"))
	if raw == "" {
		return "", fmt.Errorf("new document name must not be empty")
	}
	raw = filepath.ToSlash(raw)
	if strings.HasPrefix(raw, "/") {
		return "", fmt.Errorf("new document name %q must be relative to the vault root", value)
	}
	if ext := path.Ext(raw); strings.EqualFold(ext, ".md") {
		raw = raw[:len(raw)-len(ext)]
	}

	var rel string
	if strings.Contains(raw, "/") {
		rel = cleanRelPath(raw)
	} else {
		rel = cleanRelPath(path.Join(sourceDir, raw))
	}
	if rel == "" || rel == "." || strings.HasPrefix(rel, "../") || rel == ".." {
		return "", fmt.Errorf("new document name %q escapes the vault root", value)
	}
	for _, seg := range strings.Split(rel, "/") {
		if seg == "" || seg == "." || seg == ".." {
			return "", fmt.Errorf("new document name %q has an invalid path segment", value)
		}
		if strings.ContainsAny(seg, `\<>:"|?*`) {
			return "", fmt.Errorf("new document name %q contains invalid path characters", value)
		}
	}
	return rel, nil
}

func writeFileAtomic(filePath, content string, perm os.FileMode) error {
	if info, err := os.Stat(filePath); err == nil {
		perm = info.Mode().Perm()
	}

	dir := filepath.Dir(filePath)
	tmp, err := os.CreateTemp(dir, ".awiki-*")
	if err != nil {
		return err
	}
	tmpPath := tmp.Name()
	defer func() {
		_ = os.Remove(tmpPath)
	}()

	if _, err := tmp.WriteString(content); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := os.Chmod(tmpPath, perm); err != nil {
		return err
	}
	return os.Rename(tmpPath, filePath)
}
