use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

use super::{AvgPathReport, LintReport, Vault, WantedPage, WantedSource};
use crate::{text::lower, wiki::path::normalize_document_name};

impl Vault {
    pub(crate) fn lint(&self) -> LintReport {
        let mut report = LintReport {
            document_count: self.documents.len(),
            largest_component_size: 0,
            covered_documents: 0,
            orphans: Vec::new(),
            islands: Vec::new(),
            link_only_lines: Vec::new(),
        };
        let mut visited = vec![false; self.documents.len()];
        let mut components = Vec::new();
        for (idx, doc) in self.documents.iter().enumerate() {
            if !doc.excerpt.trim().is_empty() {
                report.covered_documents += 1;
            }
            for issue in &doc.link_only {
                report.link_only_lines.push((idx, issue.clone()));
            }
            if self.undirected[idx].is_empty() {
                report.orphans.push(idx);
                visited[idx] = true;
                continue;
            }
            if visited[idx] {
                continue;
            }
            let mut component = self.collect_component(idx, &mut visited);
            self.sort_doc_indices(&mut component);
            components.push(component);
        }
        self.sort_doc_indices(&mut report.orphans);
        report.link_only_lines.sort_by(|(a, ia), (b, ib)| {
            lower(&self.documents[*a].name)
                .cmp(&lower(&self.documents[*b].name))
                .then(ia.line.cmp(&ib.line))
        });
        components.sort_by(|a, b| {
            b.len().cmp(&a.len()).then_with(|| {
                lower(&self.documents[a[0]].name).cmp(&lower(&self.documents[b[0]].name))
            })
        });
        if components.len() > 1 {
            report.islands = components[1..].to_vec();
        }
        if let Some(first) = components.first() {
            report.largest_component_size = first.len();
        } else if !report.orphans.is_empty() {
            report.largest_component_size = 1;
        }
        report
    }

    fn collect_component(&self, start: usize, visited: &mut [bool]) -> Vec<usize> {
        let mut q = VecDeque::from([start]);
        visited[start] = true;
        let mut component = Vec::new();
        while let Some(cur) = q.pop_front() {
            component.push(cur);
            let mut neighbors: Vec<_> = self.undirected[cur].iter().copied().collect();
            self.sort_doc_indices(&mut neighbors);
            for next in neighbors {
                if !visited[next] {
                    visited[next] = true;
                    q.push_back(next);
                }
            }
        }
        component
    }

    pub(crate) fn shortest_path_indices(
        &self,
        from: usize,
        to: usize,
    ) -> Result<Vec<usize>, String> {
        if from == to {
            return Ok(vec![from]);
        }
        let mut prev = vec![usize::MAX; self.documents.len()];
        let mut q = VecDeque::from([from]);
        prev[from] = from;
        while let Some(cur) = q.pop_front() {
            let mut neighbors: Vec<_> = self.undirected[cur].iter().copied().collect();
            self.sort_doc_indices(&mut neighbors);
            for next in neighbors {
                if prev[next] != usize::MAX {
                    continue;
                }
                prev[next] = cur;
                if next == to {
                    let mut rev = Vec::new();
                    let mut at = to;
                    while at != from {
                        rev.push(at);
                        at = prev[at];
                    }
                    rev.push(from);
                    rev.reverse();
                    return Ok(rev);
                }
                q.push_back(next);
            }
        }
        Err(format!(
            "no path between {:?} and {:?}",
            self.documents[from].name, self.documents[to].name
        ))
    }

    fn largest_component_keys(&self) -> Vec<usize> {
        let mut visited = vec![false; self.documents.len()];
        let mut best: Vec<usize> = Vec::new();
        for idx in 0..self.documents.len() {
            if visited[idx] {
                continue;
            }
            if self.undirected[idx].is_empty() {
                visited[idx] = true;
                continue;
            }
            let mut component = self.collect_component(idx, &mut visited);
            self.sort_doc_indices(&mut component);
            if component.len() > best.len()
                || (component.len() == best.len()
                    && !component.is_empty()
                    && lower(&self.documents[component[0]].name)
                        < lower(&self.documents[best[0]].name))
            {
                best = component;
            }
        }
        best
    }

    pub(crate) fn approx_avg_shortest_path(
        &self,
        sample_count: usize,
        example_count: usize,
        seed: u64,
    ) -> Result<AvgPathReport, String> {
        let component = self.largest_component_keys();
        if component.len() < 2 {
            return Err(
                "largest connected component must contain at least two documents".to_string(),
            );
        }
        if sample_count == 0 {
            return Err("sample count must be positive".to_string());
        }
        let pairs = sample_component_pairs(component.len(), sample_count, seed);
        let samples: Vec<Vec<usize>> = pairs
            .par_iter()
            .map(|&(a, b)| {
                self.shortest_path_indices(component[a], component[b])
                    .unwrap_or_default()
            })
            .collect();
        let total: usize = samples.iter().map(|p| p.len().saturating_sub(1)).sum();
        let avg = total as f64 / samples.len() as f64;
        let mut longer: Vec<_> = samples
            .into_iter()
            .filter(|p| (p.len().saturating_sub(1) as f64) > avg)
            .collect();
        longer.sort_by(|a, b| {
            b.len()
                .cmp(&a.len())
                .then_with(|| path_sort_key(self, a).cmp(&path_sort_key(self, b)))
        });
        longer.truncate(example_count);
        Ok((component.len(), pairs.len(), avg, longer))
    }

    pub(crate) fn all_wanted_pages(&self) -> Vec<WantedPage> {
        #[derive(Default)]
        struct Agg {
            name: String,
            mentions: usize,
            docs: FxHashSet<usize>,
            source_index: FxHashMap<String, usize>,
            sources: Vec<WantedSource>,
        }
        let mut aggs: FxHashMap<String, Agg> = FxHashMap::default();
        for (doc_idx, doc) in self.documents.iter().enumerate() {
            for link in &doc.links {
                if link.resolved.is_some() || link.target_key.is_empty() {
                    continue;
                }
                let mut name = normalize_document_name(&link.display_target);
                if name.is_empty() {
                    name = link.display_target.trim().to_string();
                }
                if name.is_empty() {
                    continue;
                }
                let agg = aggs.entry(link.target_key.clone()).or_insert_with(|| Agg {
                    name,
                    ..Default::default()
                });
                agg.mentions += 1;
                agg.docs.insert(doc_idx);
                let context = if !link.context.trim().is_empty() {
                    link.context.trim().to_string()
                } else if !doc.excerpt.trim().is_empty() {
                    doc.excerpt.trim().to_string()
                } else {
                    "(empty)".to_string()
                };
                let key = format!("{}\0{}", doc.key, context);
                if let Some(&idx) = agg.source_index.get(&key) {
                    agg.sources[idx].mentions += 1;
                } else {
                    agg.source_index.insert(key, agg.sources.len());
                    agg.sources.push(WantedSource {
                        document: doc.name.clone(),
                        context,
                        mentions: 1,
                    });
                }
            }
        }
        let mut pages: Vec<_> = aggs
            .into_values()
            .map(|mut a| {
                a.sources.sort_by(|x, y| {
                    y.mentions
                        .cmp(&x.mentions)
                        .then_with(|| lower(&x.document).cmp(&lower(&y.document)))
                        .then_with(|| lower(&x.context).cmp(&lower(&y.context)))
                });
                WantedPage {
                    name: a.name,
                    mentions: a.mentions,
                    source_documents: a.docs.len(),
                    sources: a.sources,
                }
            })
            .collect();
        pages.sort_by(|a, b| {
            b.mentions
                .cmp(&a.mentions)
                .then(b.source_documents.cmp(&a.source_documents))
                .then_with(|| lower(&a.name).cmp(&lower(&b.name)))
        });
        pages
    }
}

fn sample_component_pairs(
    node_count: usize,
    sample_count: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    let total_pairs = node_count * (node_count - 1) / 2;
    if sample_count >= total_pairs {
        let mut pairs = Vec::with_capacity(total_pairs);
        for i in 0..node_count {
            for j in i + 1..node_count {
                pairs.push((i, j));
            }
        }
        return pairs;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut seen = FxHashSet::default();
    let mut pairs = Vec::with_capacity(sample_count);
    while pairs.len() < sample_count {
        let mut i = rng.gen_range(0..node_count);
        let mut j = rng.gen_range(0..node_count - 1);
        if j >= i {
            j += 1;
        }
        if i > j {
            std::mem::swap(&mut i, &mut j);
        }
        let key = ((i as u64) << 32) | j as u64;
        if seen.insert(key) {
            pairs.push((i, j));
        }
    }
    pairs
}

fn path_sort_key(vault: &Vault, path: &[usize]) -> String {
    path.iter()
        .map(|&idx| lower(&vault.documents[idx].name))
        .collect::<Vec<_>>()
        .join("\0")
}
