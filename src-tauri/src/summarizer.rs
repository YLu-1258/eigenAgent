use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Public API
pub fn summarize(text: &str, max_sentences: usize) -> String {
    println!("[summarizer] Received text for summarization: {}", text);
    let sentences = split_sentences(text);
    if sentences.len() <= max_sentences {
        return text.to_string();
    }

    let stopwords = stopwords();
    let word_freq = word_frequencies(text, &stopwords);

    let mut scored: Vec<(usize, f64)> = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| (i, score_sentence(s, &word_freq, &stopwords)))
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut selected: Vec<usize> = scored
        .iter()
        .take(max_sentences)
        .map(|(i, _)| *i)
        .collect();

    selected.sort();

    selected
        .into_iter()
        .map(|i| sentences[i].clone())
        .collect::<Vec<_>>()
        .join(" ")
}

// ───────────────── private helpers ─────────────────

fn split_sentences(text: &str) -> Vec<String> {
    let re = Regex::new(r"(?<=[.!?])\s+").unwrap();
    re.split(text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn word_frequencies(
    text: &str,
    stopwords: &HashSet<String>,
) -> HashMap<String, f64> {
    let mut freq = HashMap::new();
    let re = Regex::new(r"[A-Za-z]+").unwrap();

    for word in re.find_iter(text) {
        let w = word.as_str().to_lowercase();
        if !stopwords.contains(&w) {
            *freq.entry(w).or_insert(0.0) += 1.0;
        }
    }

    let max = freq.values().cloned().fold(0.0, f64::max);
    if max > 0.0 {
        for v in freq.values_mut() {
            *v /= max;
        }
    }

    freq
}

fn score_sentence(
    sentence: &str,
    freq: &HashMap<String, f64>,
    stopwords: &HashSet<String>,
) -> f64 {
    let re = Regex::new(r"[A-Za-z]+").unwrap();
    let mut score = 0.0;
    let mut count = 0.0;

    for word in re.find_iter(sentence) {
        let w = word.as_str().to_lowercase();
        if !stopwords.contains(&w) {
            if let Some(f) = freq.get(&w) {
                score += f;
                count += 1.0;
            }
        }
    }

    if count == 0.0 { 0.0 } else { score / count }
}

fn stopwords() -> HashSet<String> {
    [
        "the", "is", "and", "a", "to", "of", "in", "that", "it", "on", "for",
        "with", "as", "was", "were", "be", "by", "this", "are", "or", "an",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
