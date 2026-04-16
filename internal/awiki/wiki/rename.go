package wiki

import (
	"fmt"
	"os"
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

func Rename(root, oldIdentifier, newIdentifier string) (RenameResult, error) {
	vault, doc, newName, newPath, err := prepareRename(root, oldIdentifier, newIdentifier)
	if err != nil {
		return RenameResult{}, err
	}

	updated, touched, result, err := buildRenameUpdates(vault, doc, newName)
	if err != nil {
		return RenameResult{}, err
	}
	if err := os.Rename(doc.Path, newPath); err != nil {
		return RenameResult{}, err
	}

	moveUpdatedContent(updated, doc.Path, newPath)
	touched[newPath] = struct{}{}
	delete(touched, doc.Path)

	if err := writeUpdatedFiles(updated); err != nil {
		return RenameResult{}, err
	}

	result.FilesTouched = len(touched)
	if result.FilesTouched == 0 {
		result.FilesTouched = 1
	}
	return result, nil
}

func prepareRename(root, oldIdentifier, newIdentifier string) (vault *Vault, doc *Document, newName, newPath string, err error) {
	vault, err = Load(root)
	if err != nil {
		return nil, nil, "", "", err
	}

	doc, err = vault.ResolveDocument(oldIdentifier)
	if err != nil {
		return nil, nil, "", "", err
	}

	newName, err = validateRenameTarget(newIdentifier)
	if err != nil {
		return nil, nil, "", "", err
	}

	newKey := documentKey(newName)
	if existing, ok := vault.docsByKey[newKey]; ok && existing.Key != doc.Key {
		return nil, nil, "", "", fmt.Errorf("document %q already exists", existing.Name)
	}

	newPath = filepath.Join(vault.Root, newName+filepath.Ext(doc.Path))
	return vault, doc, newName, newPath, nil
}

func buildRenameUpdates(vault *Vault, doc *Document, newName string) (updated map[string]string, touched map[string]struct{}, result RenameResult, err error) {
	updated = make(map[string]string)
	touched = make(map[string]struct{})
	result = RenameResult{
		OldName: doc.Name,
		NewName: newName,
	}

	for _, current := range vault.Documents {
		content, err := os.ReadFile(current.Path)
		if err != nil {
			return nil, nil, RenameResult{}, err
		}

		rewritten, count := RewriteDocumentLinks(string(content), doc.Name, newName)
		changed := count > 0
		if count > 0 {
			result.LinksUpdated += count
		}

		if current.Key == doc.Key {
			var titleChanged bool
			rewritten, titleChanged = UpdateFrontMatterTitle(rewritten, doc.Name, newName)
			if titleChanged {
				changed = true
				result.TitleUpdated = true
			}
		}

		if !changed {
			continue
		}
		updated[current.Path] = rewritten
		touched[current.Path] = struct{}{}
	}

	return updated, touched, result, nil
}

func moveUpdatedContent(updated map[string]string, oldPath, newPath string) {
	if content, ok := updated[oldPath]; ok {
		delete(updated, oldPath)
		updated[newPath] = content
	}
}

func writeUpdatedFiles(updated map[string]string) error {
	for path, content := range updated {
		if err := writeFileAtomic(path, content, 0o644); err != nil {
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

func writeFileAtomic(path, content string, perm os.FileMode) error {
	if info, err := os.Stat(path); err == nil {
		perm = info.Mode().Perm()
	}

	dir := filepath.Dir(path)
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
	return os.Rename(tmpPath, path)
}
