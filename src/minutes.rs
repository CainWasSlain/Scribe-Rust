//! Rule-based meeting minutes: no AI, just word statistics and cue phrases.
use crate::transcribe::Segment;
use std::collections::{HashMap, HashSet};

// Edit these lists to tune what counts as a decision / action item.
const DECISION_CUES: &[&str] = &[
    " decided", " agreed", " agree on", " let's go with", " lets go with", " we'll go with",
    " going with ", " final decision", " approved", " confirmed", " settled on",
];
const ACTION_CUES: &[&str] = &[
    " will ", " i'll ", " we'll ", " you'll ", " he'll ", " she'll ", " they'll ", " need to ",
    " needs to ", " should ", " action item", " follow up", " follow-up", " take care of",
    " by monday", " by tuesday", " by wednesday", " by thursday", " by friday", " by tomorrow",
    " by next week", " by end of", " by eod", " deadline", " assign", " send ", " share the ",
];
const STOP: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "you", "are", "was", "have", "has", "had", "but",
    "not", "can", "will", "just", "what", "all", "they", "them", "their", "there", "then", "than",
    "from", "your", "our", "out", "about", "would", "could", "should", "been", "being", "were",
    "which", "when", "where", "who", "how", "why", "its", "it's", "i'm", "we're", "you're", "don't",
    "can't", "yeah", "yes", "okay", "right", "like", "know", "think", "going", "get", "got", "one",
    "also", "some", "any", "very", "really", "actually", "basically", "thing", "things", "here",
    "now", "said", "say", "see", "let", "let's", "into", "over", "more", "much", "maybe", "well",
    "because", "these", "those", "his", "her", "she", "him", "did", "does", "doing", "want",
    "need", "make", "sure", "mean", "kind", "lot", "good", "come", "go", "way", "too", "only",
    "even", "still", "other", "another", "we'll", "i'll", "that's", "there's", "gonna", "uh", "um",
];
const PRONOUNS: &[&str] = &["I", "We", "You", "They", "He", "She", "It", "This", "That", "So", "And", "But"];

#[derive(Debug, Clone, PartialEq)]
pub struct Sentence {
    pub seg: usize, // index into the segment list
    pub start_ms: i64,
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Minutes {
    pub topics: Vec<String>,
    pub summary: Vec<Sentence>,
    pub key_points: Vec<Sentence>,
    pub decisions: Vec<Sentence>,
    pub actions: Vec<(String, Sentence)>, // (owner, sentence)
}

/// Lowercased words with surrounding punctuation trimmed. Splits on whitespace
/// (not is_alphanumeric) because Malayalam vowel signs are combining marks.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation() || c == '…' || c == '।').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

fn content_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(&w.as_str()) && !w.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// Drops "Me" segments that are just the laptop speakers leaking into the mic:
/// overlapping in time (±2 s) with an "Others" segment and sharing ≥60% of words.
pub fn remove_echo(segs: Vec<Segment>) -> Vec<Segment> {
    let others: Vec<(i64, i64, HashSet<String>)> = segs
        .iter()
        .filter(|s| s.speaker != "Me")
        .map(|s| (s.start_ms, s.end_ms, content_words(&s.text).into_iter().collect()))
        .collect();
    segs.into_iter()
        .filter(|s| {
            if s.speaker != "Me" {
                return true;
            }
            // compare meaningful words only; "the/on/and" made real replies look like echo
            let mine: HashSet<String> = content_words(&s.text).into_iter().collect();
            if mine.is_empty() {
                return true;
            }
            !others.iter().any(|(start, end, theirs)| {
                s.start_ms <= end + 2000
                    && *start <= s.end_ms + 2000
                    && mine.intersection(theirs).count() as f32 / mine.len() as f32 >= 0.6
            })
        })
        .collect()
}

pub fn sentences(segs: &[Segment]) -> Vec<Sentence> {
    let mut out = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let mut cur = String::new();
        for c in s.text.chars() {
            cur.push(c);
            if matches!(c, '.' | '?' | '!' | '।') {
                push_sentence(&mut out, i, s, &mut cur);
            }
        }
        push_sentence(&mut out, i, s, &mut cur);
    }
    out
}

fn push_sentence(out: &mut Vec<Sentence>, i: usize, s: &Segment, cur: &mut String) {
    let t = cur.trim();
    if !t.is_empty() {
        out.push(Sentence { seg: i, start_ms: s.start_ms, speaker: s.speaker.clone(), text: t.to_string() });
    }
    cur.clear();
}

fn padded_lower(t: &str) -> String {
    format!(" {} ", t.to_lowercase().replace('’', "'"))
}

/// "Rahul will send…" -> Rahul; "I'll send…" / anything else -> the speaker label.
fn owner(s: &Sentence) -> String {
    let mut it = s.text.split_whitespace();
    if let (Some(first), Some(second)) = (it.next(), it.next()) {
        let first = first.trim_matches(|c: char| c.is_ascii_punctuation());
        let second = second.to_lowercase();
        if first.chars().next().is_some_and(|c| c.is_uppercase())
            && !PRONOUNS.contains(&first)
            && ["will", "to", "should", "needs", "can", "is"].contains(&second.as_str())
        {
            return first.to_string();
        }
    }
    s.speaker.clone()
}

pub fn build(segs: &[Segment]) -> Minutes {
    let sents = sentences(segs);
    let mut freq: HashMap<String, usize> = HashMap::new();
    for s in &sents {
        for w in content_words(&s.text) {
            *freq.entry(w).or_default() += 1;
        }
    }
    let mut topics: Vec<(&String, &usize)> = freq.iter().filter(|(_, n)| **n >= 2).collect();
    topics.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    let end = segs.iter().map(|s| s.end_ms).max().unwrap_or(1).max(1) as f32;
    let mut scored: Vec<(f32, usize)> = sents
        .iter()
        .enumerate()
        .filter(|(_, s)| words(&s.text).len() >= 5)
        .map(|(i, s)| {
            let cw = content_words(&s.text);
            // words said only once aren't meeting themes, so they add nothing
            let sum: usize = cw.iter().map(|w| freq[w]).filter(|n| *n >= 2).sum();
            let pos = s.start_ms as f32 / end;
            let boost = if !(0.1..=0.9).contains(&pos) { 1.15 } else { 1.0 }; // openings/wrap-ups matter
            (sum as f32 / (cw.len().max(1) as f32).sqrt() * boost, i)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

    let pick = |n: usize| {
        let mut idx: Vec<usize> = scored.iter().take(n).map(|(_, i)| *i).collect();
        idx.sort();
        idx.into_iter().map(|i| sents[i].clone()).collect::<Vec<_>>()
    };
    let n_summary = (sents.len() / 25).clamp(5, 8);

    let mut decisions = Vec::new();
    let mut actions = Vec::new();
    for s in &sents {
        let t = padded_lower(&s.text);
        if DECISION_CUES.iter().any(|c| t.contains(c)) {
            decisions.push(s.clone());
        } else if ACTION_CUES.iter().any(|c| t.contains(c)) && !t.trim_end().ends_with('?') {
            actions.push((owner(s), s.clone()));
        }
    }

    Minutes {
        topics: topics.into_iter().take(8).map(|(w, _)| w.clone()).collect(),
        summary: pick(n_summary),
        key_points: pick(10),
        decisions,
        actions,
    }
}

#[cfg(test)]
pub(crate) fn seg(start: i64, speaker: &str, text: &str) -> Segment {
    Segment { start_ms: start, end_ms: start + 3000, speaker: speaker.into(), lang: "en".into(), text: text.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_removed_but_real_reply_kept() {
        let segs = vec![
            seg(10_000, "Others", "The budget report is due next week for the client"),
            seg(10_500, "Me", "budget report is due next week for the client"), // echo
            seg(11_000, "Me", "Sounds good, I can handle the slides"),          // real reply
        ];
        let out = remove_echo(segs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].text, "Sounds good, I can handle the slides");
    }

    #[test]
    fn related_reply_sharing_common_words_is_not_echo() {
        let segs = vec![
            seg(0, "Others", "We decided to launch the mobile app on the 1st of December."),
            seg(1_000, "Me", "The mobile app launch depends on payment testing."),
        ];
        assert_eq!(remove_echo(segs).len(), 2);
    }

    #[test]
    fn same_words_far_apart_is_not_echo() {
        let segs = vec![seg(0, "Others", "budget report due"), seg(60_000, "Me", "budget report due")];
        assert_eq!(remove_echo(segs).len(), 2);
    }

    #[test]
    fn minutes_from_canned_meeting() {
        let segs = vec![
            seg(0, "Others", "Welcome everyone, today we review the mobile app launch plan."),
            seg(5_000, "Me", "The mobile app launch depends on the payment integration testing."),
            seg(10_000, "Others", "Did anyone watch the football match yesterday?"),
            seg(15_000, "Others", "We decided to launch the mobile app on the first of December."),
            seg(20_000, "Others", "Rahul will send the report by Friday."),
            seg(25_000, "Me", "I'll update the payment integration testing checklist for the app launch."),
            seg(30_000, "Others", "Should we invite the marketing team?"),
        ];
        let m = build(&segs);
        assert_eq!(m.decisions.len(), 1);
        assert!(m.decisions[0].text.starts_with("We decided"));
        let owners: Vec<&str> = m.actions.iter().map(|(o, _)| o.as_str()).collect();
        assert_eq!(owners, ["Rahul", "Me"]); // the question is not an action
        assert!(m.topics.iter().any(|t| t == "launch"));
        assert!(!m.summary.iter().any(|s| s.text.contains("football")), "{:?}", m.summary);
        assert!(m.summary.iter().any(|s| s.text.contains("decided to launch")));
        assert!(m.summary.windows(2).all(|w| w[0].start_ms <= w[1].start_ms)); // time order
    }

    #[test]
    fn malayalam_words_not_split_on_vowel_signs() {
        assert_eq!(words("നമസ്കാരം, എല്ലാവർക്കും."), ["നമസ്കാരം", "എല്ലാവർക്കും"]);
    }
}
