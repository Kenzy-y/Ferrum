//! Fuzzy matching for the command palette and quick open.
//!
//! A query matches a candidate when its characters appear in order
//! (case-insensitive). The score prefers matches at word starts and runs of
//! consecutive characters, so `tsb` finds "Toggle sidebar" first.

/// Points for each matched character.
const SCORE_MATCH: i32 = 1;
/// Extra points when a character continues a run of matches.
const SCORE_CONSECUTIVE: i32 = 5;
/// Extra points when a character starts a word.
const SCORE_WORD_START: i32 = 8;
/// Extra points when the match starts at the very beginning.
const SCORE_PREFIX: i32 = 4;
/// Points lost for every skipped character before the first match.
const PENALTY_LEADING_GAP: i32 = 1;

/// A successful match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Higher is better.
    pub score: i32,
    /// Byte offsets of the matched characters in the candidate, for
    /// highlighting.
    pub positions: Vec<usize>,
}

/// Matches `query` against `candidate`. An empty query matches everything
/// with score 0.
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<FuzzyMatch> {
    let query: Vec<char> = query
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            positions: Vec::new(),
        });
    }

    let mut positions = Vec::with_capacity(query.len());
    let mut score = 0;
    let mut wanted = 0;
    let mut previous: Option<char> = None;
    let mut previous_matched = false;
    for (offset, character) in candidate.char_indices() {
        if wanted == query.len() {
            break;
        }
        let matches = character.to_lowercase().eq(std::iter::once(query[wanted]));
        if matches {
            score += SCORE_MATCH;
            if previous_matched {
                score += SCORE_CONSECUTIVE;
            }
            if is_word_start(previous, character) {
                score += SCORE_WORD_START;
            }
            if offset == 0 {
                score += SCORE_PREFIX;
            }
            positions.push(offset);
            wanted += 1;
        } else if wanted == 0 {
            score -= PENALTY_LEADING_GAP;
        }
        previous_matched = matches;
        previous = Some(character);
    }
    (wanted == query.len()).then_some(FuzzyMatch { score, positions })
}

fn is_word_start(previous: Option<char>, character: char) -> bool {
    match previous {
        None => true,
        Some(previous) => {
            !previous.is_alphanumeric() || (previous.is_lowercase() && character.is_uppercase())
        }
    }
}

/// Indices of the `candidates` matching `query`, best first; ties keep the
/// original order.
pub fn filter<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<(usize, FuzzyMatch)> {
    let mut matches: Vec<(usize, FuzzyMatch)> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| fuzzy_match(query, candidate).map(|found| (index, found)))
        .collect();
    matches.sort_by(|(left_index, left), (right_index, right)| {
        right
            .score
            .cmp(&left.score)
            .then(left_index.cmp(right_index))
    });
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_subsequences_case_insensitively() {
        let found = fuzzy_match("TgSb", "Toggle sidebar").unwrap();
        assert_eq!(found.positions, vec![0, 2, 7, 11]);
        assert!(fuzzy_match("xyz", "Toggle sidebar").is_none());
        assert!(fuzzy_match("sidebar toggle", "Toggle sidebar").is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_match("", "anything").unwrap().score, 0);
        assert_eq!(fuzzy_match("  ", "anything").unwrap().score, 0);
    }

    #[test]
    fn prefers_word_starts_and_runs() {
        let candidates = ["Show Search", "Toggle sidebar", "Toggle panel"];
        let ranked: Vec<usize> = filter("ts", candidates)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        assert_eq!(ranked[0], 1, "t-oggle s-idebar beats the rest");

        let exact = fuzzy_match("panel", "Toggle panel").unwrap();
        let scattered = fuzzy_match("panel", "Open a new external link").unwrap();
        assert!(exact.score > scattered.score);
    }

    #[test]
    fn positions_are_byte_offsets() {
        let found = fuzzy_match("пн", "Показать панель").unwrap();
        let text = "Показать панель";
        assert!(
            found
                .positions
                .iter()
                .all(|offset| text.is_char_boundary(*offset))
        );
        assert_eq!(text[found.positions[1]..].chars().next(), Some('н'));
    }
}
