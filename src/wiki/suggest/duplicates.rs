use rustc_hash::FxHashMap;

use super::pages::PageStats;
use super::{NearDuplicatePair, NearDuplicateSuggestion, SuggestOptions};

struct Fingerprint {
    doc: usize,
    shingles: Vec<u64>,
    winnow: Vec<u64>,
}

pub(super) fn near_duplicates(
    stats: &[PageStats],
    opts: &SuggestOptions,
) -> NearDuplicateSuggestion {
    let fps: Vec<_> = stats
        .iter()
        .filter(|s| s.words >= opts.short_words.max(20))
        .map(Fingerprint::from_stats)
        .filter(|fp| fp.shingles.len() >= 24)
        .collect();
    let candidates = candidate_pairs(&fps);
    let mut pairs = Vec::new();
    for &(a, b) in &candidates {
        let (jaccard, containment, shared) = relation(&fps[a].shingles, &fps[b].shingles);
        let size_ratio = size_ratio(fps[a].shingles.len(), fps[b].shingles.len());
        let score = if size_ratio >= 0.6 {
            jaccard.max(containment * 0.95)
        } else {
            jaccard
        };
        if score >= opts.duplicate_threshold && shared >= 24 {
            pairs.push(NearDuplicatePair {
                a: fps[a].doc,
                b: fps[b].doc,
                score,
                containment,
                jaccard,
                shared_grams: shared,
            });
        }
    }
    pairs.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(b.shared_grams.cmp(&a.shared_grams))
    });
    pairs.truncate(opts.limit);
    NearDuplicateSuggestion {
        threshold: opts.duplicate_threshold,
        candidates: candidates.len(),
        pairs,
    }
}

impl Fingerprint {
    fn from_stats(stats: &PageStats) -> Self {
        let gram = gram_size_for(&stats.norm);
        let mut shingles = char_ngram_hashes(&stats.norm, gram);
        shingles.sort_unstable();
        shingles.dedup();
        let mut winnow = winnow(&stats.norm);
        winnow.sort_unstable();
        winnow.dedup();
        Self {
            doc: stats.doc,
            shingles,
            winnow,
        }
    }
}

fn candidate_pairs(fps: &[Fingerprint]) -> Vec<(usize, usize)> {
    let mut index: FxHashMap<u64, Vec<usize>> = FxHashMap::default();
    for (i, fp) in fps.iter().enumerate() {
        for &w in &fp.winnow {
            index.entry(w).or_default().push(i);
        }
    }
    let mut counts: FxHashMap<u64, usize> = FxHashMap::default();
    for docs in index.values() {
        if docs.len() > 64 {
            continue;
        }
        for i in 0..docs.len() {
            for j in i + 1..docs.len() {
                *counts.entry(pair_key(docs[i], docs[j])).or_default() += 1;
            }
        }
    }
    let mut pairs: Vec<_> = counts
        .into_iter()
        .filter(|&(_, count)| count >= 3)
        .map(|(key, _)| unpack_pair_key(key))
        .collect();
    pairs.sort_unstable();
    pairs
}

fn relation(a: &[u64], b: &[u64]) -> (f64, f64, usize) {
    let mut i = 0;
    let mut j = 0;
    let mut shared = 0;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let union = a.len() + b.len() - shared;
    let jaccard = if union == 0 {
        0.0
    } else {
        shared as f64 / union as f64
    };
    let min_len = a.len().min(b.len());
    let containment = if min_len == 0 {
        0.0
    } else {
        shared as f64 / min_len as f64
    };
    (jaccard, containment, shared)
}

fn char_ngram_hashes(value: &str, n: usize) -> Vec<u64> {
    let chars: Vec<_> = value.chars().collect();
    if chars.len() < n {
        return Vec::new();
    }
    (0..=chars.len() - n)
        .map(|i| stable_hash_chars(&chars[i..i + n]))
        .collect()
}

fn winnow(value: &str) -> Vec<u64> {
    const K: usize = 5;
    const W: usize = 8;
    let grams = char_ngram_hashes(value, K);
    if grams.len() < W {
        return grams.into_iter().min().into_iter().collect();
    }
    (0..=grams.len() - W)
        .map(|i| *grams[i..i + W].iter().min().unwrap())
        .collect()
}

fn stable_hash_chars(chars: &[char]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut buf = [0; 4];
    for ch in chars {
        for b in ch.encode_utf8(&mut buf).bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn gram_size_for(value: &str) -> usize {
    let chars = value.chars().count();
    let cjk = value
        .chars()
        .filter(|c| matches!(*c as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xac00..=0xd7af))
        .count();
    if cjk * 3 > chars {
        3
    } else {
        5
    }
}

fn pair_key(a: usize, b: usize) -> u64 {
    ((a.min(b) as u64) << 32) | a.max(b) as u64
}

fn unpack_pair_key(key: u64) -> (usize, usize) {
    ((key >> 32) as usize, (key & 0xffff_ffff) as usize)
}

fn size_ratio(a: usize, b: usize) -> f64 {
    if a == 0 || b == 0 {
        0.0
    } else {
        a.min(b) as f64 / a.max(b) as f64
    }
}
