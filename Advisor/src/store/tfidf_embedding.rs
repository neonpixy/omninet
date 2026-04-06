//! TF-IDF embedding provider — local-first semantic search without ML dependencies.
//!
//! Produces sparse TF-IDF vectors over a learned vocabulary. Vectors are L2-normalized
//! so that cosine similarity via [`super::embedding::cosine_similarity`] works correctly.
//!
//! This is a sovereignty-respecting starting point: zero network calls, zero model
//! downloads, zero external deps. Upgrade path: swap in `fastembed` or platform
//! embeddings (Apple NLEmbedding) behind the same [`EmbeddingProvider`] trait.

use std::collections::HashMap;

use super::embedding::EmbeddingProvider;

/// Maximum vocabulary size. Keeps vectors manageable in memory and
/// ensures cosine similarity remains meaningful.
const DEFAULT_MAX_VOCAB: usize = 512;

/// Common English stop words filtered during tokenization.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "have", "has", "do", "does",
    "did", "will", "would", "could", "should", "may", "might", "can", "shall", "to", "of", "in",
    "for", "on", "with", "at", "by", "from", "as", "into", "through", "during", "before", "after",
    "above", "below", "between", "out", "off", "over", "under", "again", "further", "then",
    "once", "here", "there", "when", "where", "why", "how", "all", "both", "each", "few", "more",
    "most", "other", "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than",
    "too", "very",
];

/// TF-IDF embedding provider.
///
/// Train on a corpus of documents to build a vocabulary and IDF weights,
/// then embed new text as a TF-IDF vector over that vocabulary.
pub struct TfIdfProvider {
    /// Ordered term list (the vocabulary).
    vocabulary: Vec<String>,
    /// Inverse document frequency per vocabulary term, same order as `vocabulary`.
    idf: Vec<f32>,
    /// Total documents seen during training.
    documents: usize,
    /// Term → vocabulary index for O(1) lookup during embedding.
    term_index: HashMap<String, usize>,
}

impl TfIdfProvider {
    /// Create a new, untrained TF-IDF provider.
    pub fn new() -> Self {
        Self {
            vocabulary: Vec::new(),
            idf: Vec::new(),
            documents: 0,
            term_index: HashMap::new(),
        }
    }

    /// Train the provider on a corpus of documents.
    ///
    /// Builds the vocabulary (top `DEFAULT_MAX_VOCAB` terms by document frequency)
    /// and computes IDF weights. Calling `train` again replaces the previous model.
    pub fn train(&mut self, documents: &[&str]) {
        if documents.is_empty() {
            return;
        }

        self.documents = documents.len();

        // Count how many documents contain each term (document frequency).
        let mut doc_freq: HashMap<String, usize> = HashMap::new();

        for doc in documents {
            let tokens = tokenize(doc);
            // Deduplicate per document — each term counts at most once per doc.
            let unique: std::collections::HashSet<&str> =
                tokens.iter().map(|s| s.as_str()).collect();
            for term in unique {
                *doc_freq.entry(term.to_string()).or_insert(0) += 1;
            }
        }

        // Select top N terms by document frequency.
        let mut term_counts: Vec<(String, usize)> = doc_freq.into_iter().collect();
        term_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        term_counts.truncate(DEFAULT_MAX_VOCAB);

        // Build vocabulary and IDF.
        let total_docs = self.documents as f32;
        self.vocabulary.clear();
        self.idf.clear();
        self.term_index.clear();

        for (i, (term, df)) in term_counts.into_iter().enumerate() {
            let idf_value = (total_docs / (1.0 + df as f32)).ln();
            self.vocabulary.push(term.clone());
            self.idf.push(idf_value);
            self.term_index.insert(term, i);
        }
    }

    /// Number of terms in the learned vocabulary.
    pub fn vocabulary_size(&self) -> usize {
        self.vocabulary.len()
    }
}

impl Default for TfIdfProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingProvider for TfIdfProvider {
    /// Embed text as a TF-IDF vector over the trained vocabulary.
    ///
    /// Returns `None` if the provider has not been trained or if the text
    /// contains no vocabulary terms (zero vector after filtering).
    fn embed(&self, text: &str) -> Option<Vec<f32>> {
        if self.vocabulary.is_empty() {
            return None;
        }

        let tokens = tokenize(text);
        if tokens.is_empty() {
            return None;
        }

        // Compute term frequency: count / total_terms.
        let total_terms = tokens.len() as f32;
        let mut term_counts: HashMap<&str, usize> = HashMap::new();
        for token in &tokens {
            *term_counts.entry(token.as_str()).or_insert(0) += 1;
        }

        // Build TF-IDF vector.
        let mut vector = vec![0.0f32; self.vocabulary.len()];
        let mut has_nonzero = false;

        for (term, count) in &term_counts {
            if let Some(&idx) = self.term_index.get(*term) {
                let tf = *count as f32 / total_terms;
                vector[idx] = tf * self.idf[idx];
                has_nonzero = true;
            }
        }

        if !has_nonzero {
            return None;
        }

        // L2-normalize so cosine similarity works correctly.
        l2_normalize(&mut vector);

        Some(vector)
    }

    fn dimension(&self) -> usize {
        self.vocabulary_size()
    }

    fn is_available(&self) -> bool {
        !self.vocabulary.is_empty()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Tokenize text: lowercase, split on non-alphanumeric, filter stop words and
/// single-character tokens.
fn tokenize(text: &str) -> Vec<String> {
    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && w.len() > 1 && !stop_set.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// L2-normalize a vector in place. No-op if magnitude is zero.
fn l2_normalize(v: &mut [f32]) {
    let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag > 0.0 {
        for x in v.iter_mut() {
            *x /= mag;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::embedding::cosine_similarity;

    fn sample_corpus() -> Vec<&'static str> {
        vec![
            "rust programming language systems performance",
            "swift programming language apple platform",
            "encrypted storage vault data security",
            "networking protocols relay tower globe",
            "design tokens layout typography color",
            "governance voting community charter rights",
            "identity crown keypair sovereignty digital",
            "rendering engine metal gpu shader",
        ]
    }

    #[test]
    fn train_builds_vocabulary() {
        let mut provider = TfIdfProvider::new();
        let corpus = sample_corpus();
        provider.train(&corpus);

        assert!(provider.vocabulary_size() > 0);
        assert!(provider.vocabulary_size() <= DEFAULT_MAX_VOCAB);
        assert_eq!(provider.idf.len(), provider.vocabulary_size());
    }

    #[test]
    fn embed_produces_correct_dimension() {
        let mut provider = TfIdfProvider::new();
        let corpus = sample_corpus();
        provider.train(&corpus);

        let embedding = provider.embed("rust systems programming").unwrap();
        assert_eq!(embedding.len(), provider.dimension());
    }

    #[test]
    fn embed_is_l2_normalized() {
        let mut provider = TfIdfProvider::new();
        let corpus = sample_corpus();
        provider.train(&corpus);

        let embedding = provider.embed("rust systems programming").unwrap();
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (magnitude - 1.0).abs() < 1e-5,
            "expected unit vector, got magnitude {magnitude}"
        );
    }

    #[test]
    fn similar_texts_have_higher_cosine_similarity() {
        let mut provider = TfIdfProvider::new();
        let corpus = sample_corpus();
        provider.train(&corpus);

        let rust_vec = provider.embed("rust programming systems").unwrap();
        let swift_vec = provider.embed("swift programming apple").unwrap();
        let vault_vec = provider.embed("encrypted storage security").unwrap();

        let rust_swift = cosine_similarity(&rust_vec, &swift_vec);
        let rust_vault = cosine_similarity(&rust_vec, &vault_vec);

        // "rust programming" and "swift programming" share "programming",
        // while "encrypted storage" shares nothing with rust.
        assert!(
            rust_swift > rust_vault,
            "expected rust-swift ({rust_swift:.4}) > rust-vault ({rust_vault:.4})"
        );
    }

    #[test]
    fn stop_words_are_filtered() {
        let mut provider = TfIdfProvider::new();
        provider.train(&["the quick brown fox"]);

        // "the" is a stop word, should not appear in vocabulary.
        assert!(!provider.vocabulary.contains(&"the".to_string()));
        // "quick", "brown", "fox" should be present.
        assert!(provider.vocabulary.contains(&"quick".to_string()));
        assert!(provider.vocabulary.contains(&"brown".to_string()));
        assert!(provider.vocabulary.contains(&"fox".to_string()));
    }

    #[test]
    fn empty_text_returns_none() {
        let mut provider = TfIdfProvider::new();
        provider.train(&sample_corpus());

        assert!(provider.embed("").is_none());
    }

    #[test]
    fn only_stop_words_returns_none() {
        let mut provider = TfIdfProvider::new();
        provider.train(&sample_corpus());

        assert!(provider.embed("the a an is are was").is_none());
    }

    #[test]
    fn untrained_provider_returns_none() {
        let provider = TfIdfProvider::new();
        assert!(provider.embed("anything").is_none());
        assert!(!provider.is_available());
    }

    #[test]
    fn trained_provider_is_available() {
        let mut provider = TfIdfProvider::new();
        provider.train(&sample_corpus());
        assert!(provider.is_available());
    }

    #[test]
    fn train_on_empty_corpus_is_noop() {
        let mut provider = TfIdfProvider::new();
        provider.train(&[]);
        assert_eq!(provider.vocabulary_size(), 0);
        assert!(!provider.is_available());
    }

    #[test]
    fn unknown_terms_return_none() {
        let mut provider = TfIdfProvider::new();
        provider.train(&["alpha beta gamma"]);

        // None of these terms are in the vocabulary.
        assert!(provider.embed("zzzz yyyy xxxx").is_none());
    }

    #[test]
    fn identical_texts_have_similarity_near_one() {
        let mut provider = TfIdfProvider::new();
        provider.train(&sample_corpus());

        let v1 = provider.embed("rust programming language").unwrap();
        let v2 = provider.embed("rust programming language").unwrap();
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "expected ~1.0 for identical texts, got {sim}"
        );
    }

    #[test]
    fn retrain_replaces_previous_model() {
        let mut provider = TfIdfProvider::new();
        provider.train(&["alpha beta gamma"]);
        let size_1 = provider.vocabulary_size();

        provider.train(&["delta epsilon zeta eta theta"]);
        let size_2 = provider.vocabulary_size();

        // The vocabulary should change.
        assert!(!provider.vocabulary.contains(&"alpha".to_string()));
        assert!(provider.vocabulary.contains(&"delta".to_string()));
        // Sizes may differ depending on token counts.
        assert!(size_1 > 0);
        assert!(size_2 > 0);
    }

    #[test]
    fn vocabulary_capped_at_max() {
        let mut provider = TfIdfProvider::new();
        // Generate a corpus with many unique terms.
        let terms: Vec<String> = (0..1000).map(|i| format!("term{i}")).collect();
        let doc = terms.join(" ");
        provider.train(&[&doc]);

        assert!(
            provider.vocabulary_size() <= DEFAULT_MAX_VOCAB,
            "vocabulary size {} exceeds max {}",
            provider.vocabulary_size(),
            DEFAULT_MAX_VOCAB
        );
    }

    #[test]
    fn single_char_tokens_filtered() {
        let mut provider = TfIdfProvider::new();
        provider.train(&["x y z real words here"]);

        // Single-char tokens should be filtered.
        assert!(!provider.vocabulary.contains(&"x".to_string()));
        assert!(!provider.vocabulary.contains(&"y".to_string()));
        assert!(!provider.vocabulary.contains(&"z".to_string()));
        // Multi-char tokens should be present.
        assert!(provider.vocabulary.contains(&"real".to_string()));
        assert!(provider.vocabulary.contains(&"words".to_string()));
    }
}
