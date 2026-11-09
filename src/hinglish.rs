//! Hindi (Devanagari) -> Hinglish (Hindi in English letters), rule-based.
//! Anything that isn't Devanagari (English, Malayalam, punctuation) passes through unchanged.
//!
//! Spelling follows how people type Hindi in chats: "main", "nahin", "karna", "kaam".
//! The silent inherent "a" is dropped at the end of words and in the middle where Hindi
//! speakers don't say it (karana -> karna, samajhana -> samajhna).

const VIRAMA: char = '\u{094D}';
const NUKTA: char = '\u{093C}';

fn consonant(c: char) -> Option<&'static str> {
    Some(match c {
        'क' => "k", 'ख' => "kh", 'ग' => "g", 'घ' => "gh", 'ङ' => "n",
        'च' => "ch", 'छ' => "chh", 'ज' => "j", 'झ' => "jh", 'ञ' => "n",
        'ट' => "t", 'ठ' => "th", 'ड' => "d", 'ढ' => "dh", 'ण' => "n",
        'त' => "t", 'थ' => "th", 'द' => "d", 'ध' => "dh", 'न' => "n",
        'प' => "p", 'फ' => "ph", 'ब' => "b", 'भ' => "bh", 'म' => "m",
        'य' => "y", 'र' => "r", 'ल' => "l", 'व' => "v",
        'श' => "sh", 'ष' => "sh", 'स' => "s", 'ह' => "h",
        // precomposed nukta letters: qa khha ghha za dddha rha fa yya
        '\u{0958}' => "q", '\u{0959}' => "kh", '\u{095A}' => "gh", '\u{095B}' => "z",
        '\u{095C}' => "r", '\u{095D}' => "rh", '\u{095E}' => "f", '\u{095F}' => "y",
        _ => return None,
    })
}

/// Consonant + combining nukta (the decomposed form of the letters above).
fn with_nukta(c: char) -> Option<&'static str> {
    Some(match c {
        'क' => "q", 'ख' => "kh", 'ग' => "gh", 'ज' => "z", 'ड' => "r", 'ढ' => "rh", 'फ' => "f",
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum V {
    Inherent,      // unwritten "a", may be silent
    Dropped,       // virama, or an inherent "a" judged silent
    Long,          // aa (ा / आ)
    Other(&'static str),
}

fn vowel_sign(c: char) -> Option<V> {
    Some(match c {
        'ा' => V::Long,
        'ि' | 'ी' => V::Other("i"),
        'ु' | 'ू' => V::Other("u"),
        'ृ' => V::Other("ri"),
        'े' | 'ॅ' => V::Other("e"),
        'ै' => V::Other("ai"),
        'ो' | 'ॉ' => V::Other("o"),
        'ौ' => V::Other("au"),
        _ => return None,
    })
}

fn independent_vowel(c: char) -> Option<V> {
    Some(match c {
        'अ' => V::Other("a"),
        'आ' => V::Long,
        'इ' | 'ई' => V::Other("i"),
        'उ' | 'ऊ' => V::Other("u"),
        'ऋ' => V::Other("ri"),
        'ए' | 'ऍ' => V::Other("e"),
        'ऐ' => V::Other("ai"),
        'ओ' | 'ऑ' => V::Other("o"),
        'औ' => V::Other("au"),
        _ => return None,
    })
}

/// One consonant (or a bare vowel) with its vowel and any nasal/visarga after it.
struct Unit {
    cons: &'static str,
    vowel: V,
    tail: &'static str,
}

fn is_devanagari(c: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&c) && !matches!(c, '।' | '॥' | '०'..='९')
}

pub fn to_hinglish(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if is_devanagari(chars[i]) {
            let start = i;
            while i < chars.len() && (is_devanagari(chars[i]) || matches!(chars[i], '\u{200C}' | '\u{200D}')) {
                i += 1;
            }
            out.push_str(&word(&chars[start..i]));
            continue;
        }
        match chars[i] {
            '।' | '॥' => out.push('.'),
            c @ '०'..='९' => out.push(char::from(b'0' + (c as u32 - '०' as u32) as u8)),
            c => out.push(c),
        }
        i += 1;
    }
    out
}

fn word(chars: &[char]) -> String {
    let mut units: Vec<Unit> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if let Some(mut cons) = consonant(c) {
            if chars.get(i) == Some(&NUKTA) {
                cons = with_nukta(c).unwrap_or(cons);
                i += 1;
            }
            let vowel = match chars.get(i) {
                Some(&VIRAMA) => {
                    i += 1;
                    V::Dropped
                }
                Some(&v) => vowel_sign(v).inspect(|_| i += 1).unwrap_or(V::Inherent),
                None => V::Inherent,
            };
            units.push(Unit { cons, vowel, tail: "" });
        } else if let Some(vowel) = independent_vowel(c) {
            units.push(Unit { cons: "", vowel, tail: "" });
        } else if let Some(last) = units.last_mut() {
            match c {
                'ं' | 'ँ' => {
                    // anusvara sounds "m" before p/b/m: sambandh, hamesha stays as is
                    let labial = chars.get(i).and_then(|n| consonant(*n)).is_some_and(|n| {
                        matches!(n, "p" | "ph" | "b" | "bh" | "m")
                    });
                    last.tail = if c == 'ं' && labial { "m" } else { "n" };
                }
                'ः' => last.tail = "h",
                _ => {} // ZWJ/ZWNJ, stray signs
            }
        }
    }

    // ज्ञ is said "gy": gyaan
    for k in 1..units.len() {
        if units[k - 1].cons == "j" && units[k - 1].vowel == V::Dropped && consonant('ञ') == Some(units[k].cons) {
            units[k - 1].cons = "g";
            units[k].cons = "y";
        }
    }

    // Silent inherent "a", right to left.
    let n = units.len();
    let sounds = |u: &Unit| u.vowel != V::Dropped;
    if n > 1 && units[n - 1].vowel == V::Inherent && units[n - 1].tail.is_empty() {
        // keep it after a cluster ending in r/y/v (mitra, satya), drop it otherwise (kaam, riport)
        let cluster = units[n - 2].vowel == V::Dropped && matches!(units[n - 1].cons, "r" | "y" | "v");
        if !cluster {
            units[n - 1].vowel = V::Dropped;
        }
    }
    for k in (1..n.saturating_sub(1)).rev() {
        if units[k].vowel == V::Inherent
            && units[k].tail.is_empty()
            && !units[k].cons.is_empty()
            && sounds(&units[k - 1])
            && units[k + 1].vowel != V::Dropped
            && !units[k + 1].cons.is_empty()
        {
            units[k].vowel = V::Dropped;
        }
    }

    // "aa" in the first syllable when more follows (kaam, aaj, jaana), "a" elsewhere (kya, hamara).
    let first_vowel = units.iter().position(|u| sounds(u));
    let mut s = String::new();
    for (k, u) in units.iter().enumerate() {
        s.push_str(u.cons);
        match u.vowel {
            V::Inherent => s.push('a'),
            V::Dropped => {}
            V::Long => {
                let more = k + 1 < n || !u.tail.is_empty();
                s.push_str(if (Some(k) == first_vowel && more) || u.cons.is_empty() { "aa" } else { "a" });
            }
            V::Other(v) => s.push_str(v),
        }
        s.push_str(u.tail);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::to_hinglish as h;

    #[test]
    fn common_words() {
        for (hindi, hinglish) in [
            ("मैं", "main"), ("है", "hai"), ("हैं", "hain"), ("नहीं", "nahin"), ("क्या", "kya"),
            ("काम", "kaam"), ("बात", "baat"), ("आज", "aaj"), ("आप", "aap"), ("हम", "ham"),
            ("करना", "karna"), ("समझना", "samajhna"), ("अपना", "apna"), ("हमारा", "hamara"),
            ("जाना", "jaana"), ("था", "tha"), ("रहे", "rahe"), ("करते", "karte"), ("हाँ", "haan"),
            ("मित्र", "mitra"), ("ज़रूर", "zarur"), ("ज्ञान", "gyaan"), ("संबंध", "sambandh"),
            ("भेजेंगे", "bhejenge"), ("सत्य", "satya"), ("कार्य", "kaarya"), ("धर्म", "dharm"), ("कल", "kal"), ("रिपोर्ट", "riport"),
        ] {
            assert_eq!(h(hindi), hinglish, "{hindi}");
        }
    }

    #[test]
    fn sentences_and_other_scripts_pass_through() {
        assert_eq!(h("मैं कल report भेजूँगा।"), "main kal report bhejunga.");
        assert_eq!(h("Meeting at 5 PM."), "Meeting at 5 PM.");
        assert_eq!(h("ente report"), "ente report");
        assert_eq!(h("നാളെ"), "നാളെ"); // Malayalam is left for manglish
    }
}
