package wiki

import (
	"fmt"
	"math/rand"
	"sort"
	"strings"
)

type ShortestPathSample struct {
	Nodes  []string
	Length int
}

type AvgShortestPathReport struct {
	ComponentSize int
	SampleCount   int
	Average       float64
	LongerPaths   []ShortestPathSample
}

func (v *Vault) ApproximateAverageShortestPath(sampleCount, exampleCount int, seed int64) (AvgShortestPathReport, error) {
	component := v.largestConnectedComponentKeys()
	if len(component) < 2 {
		return AvgShortestPathReport{}, fmt.Errorf("largest connected component must contain at least two documents")
	}
	if sampleCount <= 0 {
		return AvgShortestPathReport{}, fmt.Errorf("sample count must be positive")
	}

	pairs := sampleComponentPairs(len(component), sampleCount, seed)
	samples := make([]ShortestPathSample, 0, len(pairs))
	totalLength := 0

	for _, pair := range pairs {
		path, err := v.shortestPathBetweenKeys(component[pair[0]], component[pair[1]])
		if err != nil {
			return AvgShortestPathReport{}, err
		}

		length := len(path) - 1
		totalLength += length
		samples = append(samples, ShortestPathSample{
			Nodes:  path,
			Length: length,
		})
	}

	report := AvgShortestPathReport{
		ComponentSize: len(component),
		SampleCount:   len(samples),
		Average:       float64(totalLength) / float64(len(samples)),
	}
	if exampleCount <= 0 {
		return report, nil
	}

	for _, sample := range samples {
		if float64(sample.Length) > report.Average {
			report.LongerPaths = append(report.LongerPaths, sample)
		}
	}
	sort.Slice(report.LongerPaths, func(i, j int) bool {
		if report.LongerPaths[i].Length != report.LongerPaths[j].Length {
			return report.LongerPaths[i].Length > report.LongerPaths[j].Length
		}
		return strings.ToLower(strings.Join(report.LongerPaths[i].Nodes, "\x00")) < strings.ToLower(strings.Join(report.LongerPaths[j].Nodes, "\x00"))
	})
	if len(report.LongerPaths) > exampleCount {
		report.LongerPaths = report.LongerPaths[:exampleCount]
	}

	return report, nil
}

func sampleComponentPairs(nodeCount, sampleCount int, seed int64) [][2]int {
	totalPairs := nodeCount * (nodeCount - 1) / 2
	if sampleCount >= totalPairs {
		pairs := make([][2]int, 0, totalPairs)
		for i := 0; i < nodeCount; i++ {
			for j := i + 1; j < nodeCount; j++ {
				pairs = append(pairs, [2]int{i, j})
			}
		}
		return pairs
	}

	rng := rand.New(rand.NewSource(seed))
	seen := make(map[uint64]struct{}, sampleCount)
	pairs := make([][2]int, 0, sampleCount)
	for len(pairs) < sampleCount {
		i := rng.Intn(nodeCount)
		j := rng.Intn(nodeCount - 1)
		if j >= i {
			j++
		}
		if i > j {
			i, j = j, i
		}

		key := uint64(i)<<32 | uint64(j)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		pairs = append(pairs, [2]int{i, j})
	}
	return pairs
}
