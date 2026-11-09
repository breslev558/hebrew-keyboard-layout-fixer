//! 1.8.2, letter models (character trigrams) for words that are in no list: "does this look like
//! Hebrew / English?". Built on the user's machine from the word lists already downloaded (no
//! new data file), so they cost nothing to ship and follow the lists' licence. Pure and tested.

use std::collections::HashMap;

/// Start / end of word.
const EDGE: char = '\u{1}';
/// Smoothing: a sequence never seen still gets a small chance.
const K: f64 = 0.05;

#[derive(Debug, Default)]
pub struct Model {
    tri: HashMap<(char, char, char), f64>,
    ctx: HashMap<(char, char), f64>,
    alphabet: f64,
}

impl Model {
    /// `words`: most frequent first; frequent words weigh a little more than rare ones.
    pub fn train<'a>(words: impl Iterator<Item = &'a str>) -> Model {
        let mut m = Model::default();
        let mut letters = std::collections::HashSet::new();
        for (rank, w) in words.enumerate() {
            let weight = 1.0 + (1.0 + 10_000.0 / (rank as f64 + 100.0)).ln();
            let chars: Vec<char> = std::iter::once(EDGE).chain(std::iter::once(EDGE)).chain(w.chars().flat_map(char::to_lowercase)).chain(std::iter::once(EDGE)).collect();
            for t in chars.windows(3) {
                letters.insert(t[2]);
                *m.tri.entry((t[0], t[1], t[2])).or_default() += weight;
                *m.ctx.entry((t[0], t[1])).or_default() += weight;
            }
        }
        m.alphabet = letters.len().max(1) as f64;
        m
    }
    pub fn is_empty(&self) -> bool {
        self.tri.is_empty()
    }
    /// Average log-probability per letter (end included): higher = looks more like the language.
    pub fn score(&self, w: &str) -> f64 {
        let chars: Vec<char> = std::iter::once(EDGE).chain(std::iter::once(EDGE)).chain(w.chars().flat_map(char::to_lowercase)).chain(std::iter::once(EDGE)).collect();
        let mut sum = 0.0;
        let mut n = 0.0;
        for t in chars.windows(3) {
            let c = self.tri.get(&(t[0], t[1], t[2])).copied().unwrap_or(0.0);
            let x = self.ctx.get(&(t[0], t[1])).copied().unwrap_or(0.0);
            sum += ((c + K) / (x + K * self.alphabet)).ln();
            n += 1.0;
        }
        if n == 0.0 { f64::NEG_INFINITY } else { sum / n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_words_score_better_than_noise() {
        let en = Model::train(["the", "there", "then", "other", "mother", "brother", "think", "thing"].into_iter());
        assert!(en.score("theirs") > en.score("xqzv"));
        assert!(en.score("Thing") == en.score("thing"));
        assert!(Model::default().is_empty());
    }
}
