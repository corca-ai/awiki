mod duplicates;
mod pages;

use rayon::prelude::*;

use super::{analysis::path_sort_key, analysis::sample_component_pairs, Vault, WantedPage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SuggestFilter {
    SampledDiameter,
    WantedPressure,
    LongPages,
    ShortStubs,
    NearDuplicates,
}

impl SuggestFilter {
    pub(crate) const ALL: [Self; 5] = [
        Self::SampledDiameter,
        Self::WantedPressure,
        Self::LongPages,
        Self::ShortStubs,
        Self::NearDuplicates,
    ];

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "sampled-diameter" => Ok(Self::SampledDiameter),
            "wanted-pressure" => Ok(Self::WantedPressure),
            "long-pages" => Ok(Self::LongPages),
            "short-stubs" => Ok(Self::ShortStubs),
            "near-duplicates" => Ok(Self::NearDuplicates),
            _ => Err(format!("unknown suggest filter: {value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::SampledDiameter => "sampled-diameter",
            Self::WantedPressure => "wanted-pressure",
            Self::LongPages => "long-pages",
            Self::ShortStubs => "short-stubs",
            Self::NearDuplicates => "near-duplicates",
        }
    }
}

#[derive(Clone)]
pub(crate) struct SuggestOptions {
    pub(crate) filters: Vec<SuggestFilter>,
    pub(crate) samples: usize,
    pub(crate) paths: usize,
    pub(crate) limit: usize,
    pub(crate) seed: u64,
    pub(crate) long_lines: usize,
    pub(crate) long_words: usize,
    pub(crate) short_words: usize,
    pub(crate) duplicate_threshold: f64,
}

impl Default for SuggestOptions {
    fn default() -> Self {
        Self {
            filters: SuggestFilter::ALL.to_vec(),
            samples: 2_000,
            paths: 5,
            limit: 10,
            seed: 1,
            long_lines: 120,
            long_words: 1_200,
            short_words: 40,
            duplicate_threshold: 0.82,
        }
    }
}

pub(crate) struct SuggestReport {
    pub(crate) filters: Vec<SuggestFilter>,
    pub(crate) sampled_diameter: Option<SampledDiameterSuggestion>,
    pub(crate) wanted_pressure: Option<Vec<WantedPage>>,
    pub(crate) long_pages: Option<PageLengthSuggestion>,
    pub(crate) short_stubs: Option<PageLengthSuggestion>,
    pub(crate) near_duplicates: Option<NearDuplicateSuggestion>,
}

pub(crate) struct SampledDiameterSuggestion {
    pub(crate) samples: usize,
    pub(crate) sampled_diameter: usize,
    pub(crate) paths: Vec<Vec<usize>>,
}

pub(crate) struct PageLengthSuggestion {
    pub(crate) line_threshold: Option<usize>,
    pub(crate) word_threshold: usize,
    pub(crate) rate: f64,
    pub(crate) pages: Vec<PageLengthHit>,
}

pub(crate) struct PageLengthHit {
    pub(crate) doc: usize,
    pub(crate) lines: usize,
    pub(crate) words: usize,
}

pub(crate) struct NearDuplicateSuggestion {
    pub(crate) threshold: f64,
    pub(crate) candidates: usize,
    pub(crate) pairs: Vec<NearDuplicatePair>,
}

pub(crate) struct NearDuplicatePair {
    pub(crate) a: usize,
    pub(crate) b: usize,
    pub(crate) score: f64,
    pub(crate) containment: f64,
    pub(crate) jaccard: f64,
    pub(crate) shared_grams: usize,
}

impl Vault {
    pub(crate) fn suggest(&self, opts: &SuggestOptions) -> Result<SuggestReport, String> {
        let wants_lengths = opts.filters.iter().any(|f| {
            matches!(
                f,
                SuggestFilter::LongPages
                    | SuggestFilter::ShortStubs
                    | SuggestFilter::NearDuplicates
            )
        });
        let page_stats = if wants_lengths {
            Some(pages::collect_page_stats(self)?)
        } else {
            None
        };

        Ok(SuggestReport {
            filters: opts.filters.clone(),
            sampled_diameter: opts
                .filters
                .contains(&SuggestFilter::SampledDiameter)
                .then(|| self.sampled_diameter(opts)),
            wanted_pressure: opts
                .filters
                .contains(&SuggestFilter::WantedPressure)
                .then(|| {
                    let mut pages = self.all_wanted_pages();
                    pages.truncate(opts.limit);
                    pages
                }),
            long_pages: opts
                .filters
                .contains(&SuggestFilter::LongPages)
                .then(|| pages::long_pages(page_stats.as_ref().unwrap(), opts)),
            short_stubs: opts
                .filters
                .contains(&SuggestFilter::ShortStubs)
                .then(|| pages::short_stubs(page_stats.as_ref().unwrap(), opts)),
            near_duplicates: opts
                .filters
                .contains(&SuggestFilter::NearDuplicates)
                .then(|| duplicates::near_duplicates(page_stats.as_ref().unwrap(), opts)),
        })
    }

    fn sampled_diameter(&self, opts: &SuggestOptions) -> SampledDiameterSuggestion {
        let component = self.largest_component_keys();
        let pairs = if component.len() < 2 || opts.samples == 0 {
            Vec::new()
        } else {
            sample_component_pairs(component.len(), opts.samples, opts.seed)
        };
        let mut paths: Vec<_> = pairs
            .par_iter()
            .filter_map(|&(a, b)| self.shortest_path_indices(component[a], component[b]).ok())
            .collect();
        paths.sort_by(|a, b| {
            b.len()
                .cmp(&a.len())
                .then_with(|| path_sort_key(self, a).cmp(&path_sort_key(self, b)))
        });
        paths.truncate(opts.paths);
        let sampled_diameter = paths
            .first()
            .map(|path| path.len().saturating_sub(1))
            .unwrap_or_default();
        SampledDiameterSuggestion {
            samples: pairs.len(),
            sampled_diameter,
            paths,
        }
    }
}

pub(crate) fn format_suggest_report(vault: &Vault, report: &SuggestReport) -> String {
    let filters = report
        .filters
        .iter()
        .map(|f| f.name())
        .collect::<Vec<_>>()
        .join(",");
    let mut out = format!(
        "// suggest documents={} filters={filters}",
        vault.documents.len()
    );
    if let Some(s) = &report.sampled_diameter {
        format_sampled_diameter(vault, s, &mut out);
    }
    if let Some(pages) = &report.wanted_pressure {
        format_wanted_pressure(pages, &mut out);
    }
    if let Some(pages) = &report.long_pages {
        format_page_length(vault, "long_pages", pages, &mut out);
    }
    if let Some(pages) = &report.short_stubs {
        format_page_length(vault, "short_stubs", pages, &mut out);
    }
    if let Some(dups) = &report.near_duplicates {
        format_near_duplicates(vault, dups, &mut out);
    }
    out
}

fn format_sampled_diameter(vault: &Vault, s: &SampledDiameterSuggestion, out: &mut String) {
    out.push_str(&format!(
        "\n// sampled_diameter samples={} sampled_diameter={} paths={}",
        s.samples,
        s.sampled_diameter,
        s.paths.len()
    ));
    append_guidance(
        out,
        "these sampled paths are long, so related topics may only connect through many weak hops.",
        "inspect the endpoints and middle bridge pages; add contextual links or a bridge note where the relationship is real.",
        "if two endpoints are clearly related, add a sentence-level link from the stronger overview page.",
    );
    for path in &s.paths {
        out.push_str(&format!(
            "\n// path distance={}",
            path.len().saturating_sub(1)
        ));
        for &idx in path {
            out.push('\n');
            out.push_str(&vault.document_line(idx));
        }
    }
}

fn format_wanted_pressure(pages: &[WantedPage], out: &mut String) {
    out.push_str(&format!("\n// wanted_pressure pages={}", pages.len()));
    append_guidance(
        out,
        "many notes point at these missing pages, so the wiki already depends on absent concepts.",
        "create the page, correct misspelled links, or consolidate equivalent names with a rename/redirect note.",
        "if [[Vector database]] is linked from many pages, create it as a hub or correct links to an existing page.",
    );
    if pages.is_empty() {
        out.push_str("\n_ none");
        return;
    }
    for page in pages {
        out.push_str(&format!(
            "\n[[{}]] mentions={} source_documents={}",
            page.name, page.mentions, page.source_documents
        ));
        for source in page.sources.iter().take(3) {
            out.push_str(&format!("\n- [[{}]]: {}", source.document, source.context));
        }
    }
}

fn format_page_length(vault: &Vault, label: &str, pages: &PageLengthSuggestion, out: &mut String) {
    match pages.line_threshold {
        Some(lines) => out.push_str(&format!(
            "\n// {label} threshold_lines={} threshold_words={} rate={:.4} pages={}",
            lines,
            pages.word_threshold,
            pages.rate,
            pages.pages.len()
        )),
        None => out.push_str(&format!(
            "\n// {label} threshold_words={} rate={:.4} pages={}",
            pages.word_threshold,
            pages.rate,
            pages.pages.len()
        )),
    }
    append_page_length_guidance(label, out);
    if pages.pages.is_empty() {
        out.push_str("\n_ none");
        return;
    }
    for hit in &pages.pages {
        out.push_str(&format!(
            "\n{} lines={} words={}",
            vault.document_line(hit.doc),
            hit.lines,
            hit.words
        ));
    }
}

fn format_near_duplicates(vault: &Vault, dups: &NearDuplicateSuggestion, out: &mut String) {
    out.push_str(&format!(
        "\n// near_duplicates threshold={:.2} candidates={} pairs={}",
        dups.threshold,
        dups.candidates,
        dups.pairs.len()
    ));
    append_guidance(
        out,
        "these pages share unusually similar text, which can split links and let two versions drift apart.",
        "compare both pages, merge the weaker one into the stronger one, then rename or update links as needed.",
        "keep the clearer page, move any unique detail into it, then update links from the weaker page.",
    );
    if dups.pairs.is_empty() {
        out.push_str("\n_ none");
        return;
    }
    for pair in &dups.pairs {
        out.push_str(&format!(
            "\n[[{}]] <-> [[{}]] score={:.3} containment={:.3} jaccard={:.3} shared_grams={}",
            vault.documents[pair.a].name,
            vault.documents[pair.b].name,
            pair.score,
            pair.containment,
            pair.jaccard,
            pair.shared_grams
        ));
        out.push_str(&format!("\n- {}", vault.document_line(pair.a)));
        out.push_str(&format!("\n- {}", vault.document_line(pair.b)));
    }
}

fn append_page_length_guidance(label: &str, out: &mut String) {
    match label {
        "long_pages" => append_guidance(
            out,
            "long pages often mix several concepts, making them harder to scan, link to, and maintain.",
            "split stable subtopics into separate pages, then leave a short summary with links.",
            "split \"Machine learning\" into \"Supervised learning\", \"Evaluation metrics\", and a short overview page.",
        ),
        "short_stubs" => append_guidance(
            out,
            "very short pages may add navigation cost without enough context to justify a separate note.",
            "expand the note, merge it into a stronger related page, or delete it if it is only a placeholder.",
            "merge \"Backpropagation note\" into [[Backpropagation]] unless it has a distinct role.",
        ),
        _ => {}
    }
}

fn append_guidance(out: &mut String, why: &str, fix: &str, example: &str) {
    out.push_str(&format!(
        "\n// why: {why}\n// fix: {fix}\n// example: {example}"
    ));
}
