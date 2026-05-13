package wiki

import (
	"sort"
	"strings"
)

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

func ratio(count, total int) float64 {
	if total == 0 {
		return 0
	}
	return float64(count) / float64(total)
}
