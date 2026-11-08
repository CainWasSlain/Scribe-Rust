//! Malayalam script -> Manglish (Malayalam in English letters), rule-based.
//! Anything that isn't Malayalam (English, punctuation) passes through unchanged.

const VIRAMA: char = '\u{0D4D}';
const ZWJ: char = '\u{200D}';
const ZWNJ: char = '\u{200C}';

// Common conjuncts spelled the way people type them, checked before single letters.
const CONJUNCTS: &[(&str, &str)] = &[
    ("ന്റ", "nt"), ("ങ്ങ", "ng"), ("ഞ്ഞ", "nj"), ("റ്റ", "tt"), ("ത്ത", "tth"), ("ന്ന", "nn"),
];

fn consonant(c: char) -> Option<&'static str> {
    Some(match c {
        'ക' => "k", 'ഖ' => "kh", 'ഗ' => "g", 'ഘ' => "gh", 'ങ' => "ng",
        'ച' => "ch", 'ഛ' => "chh", 'ജ' => "j", 'ഝ' => "jh", 'ഞ' => "nj",
        'ട' => "t", 'ഠ' => "tt", 'ഡ' => "d", 'ഢ' => "dd", 'ണ' => "n",
        'ത' => "th", 'ഥ' => "th", 'ദ' => "d", 'ധ' => "dh", 'ന' => "n",
        'പ' => "p", 'ഫ' => "ph", 'ബ' => "b", 'ഭ' => "bh", 'മ' => "m",
        'യ' => "y", 'ര' => "r", 'റ' => "r", 'ല' => "l", 'ള' => "l", 'ഴ' => "zh",
        'വ' => "v", 'ശ' => "sh", 'ഷ' => "sh", 'സ' => "s", 'ഹ' => "h",
        _ => return None,
    })
}

fn vowel_sign(c: char) -> Option<&'static str> {
    Some(match c {
        'ാ' => "aa", 'ി' => "i", 'ീ' => "ee", 'ു' => "u", 'ൂ' => "oo", 'ൃ' => "ru",
        'െ' => "e", 'േ' => "e", 'ൈ' => "ai", 'ൊ' => "o", 'ോ' => "o", 'ൌ' | 'ൗ' => "au",
        _ => return None,
    })
}

fn other(c: char) -> Option<&'static str> {
    Some(match c {
        'അ' => "a", 'ആ' => "aa", 'ഇ' => "i", 'ഈ' => "ee", 'ഉ' => "u", 'ഊ' => "oo", 'ഋ' => "ru",
        'എ' | 'ഏ' => "e", 'ഐ' => "ai", 'ഒ' | 'ഓ' => "o", 'ഔ' => "au",
        'ം' => "m", 'ഃ' => "h",
        'ൺ' | 'ൻ' => "n", 'ർ' => "r", 'ൽ' | 'ൾ' => "l", 'ൿ' => "k", // chillu letters
        '൦' => "0", '൧' => "1", '൨' => "2", '൩' => "3", '൪' => "4",
        '൫' => "5", '൬' => "6", '൭' => "7", '൮' => "8", '൯' => "9",
        ZWJ | ZWNJ => "",
        _ => return None,
    })
}

fn is_malayalam_letter(c: Option<&char>) -> bool {
    c.is_some_and(|c| ('\u{0D00}'..='\u{0D7F}').contains(c) && *c != VIRAMA)
}

pub fn to_manglish(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let conj = CONJUNCTS.iter().find(|(m, _)| {
            let m: Vec<char> = m.chars().collect();
            chars[i..].starts_with(&m)
        });
        let (cons, len) = match (conj, consonant(chars[i])) {
            (Some((_, latin)), _) => (*latin, 3),
            (None, Some(latin)) => (latin, 1),
            (None, None) => {
                match other(chars[i]) {
                    Some(latin) => out.push_str(latin),
                    None => out.push(chars[i]),
                }
                i += 1;
                continue;
            }
        };
        out.push_str(cons);
        i += len;
        match chars.get(i) {
            Some(&c) if vowel_sign(c).is_some() => {
                out.push_str(vowel_sign(c).unwrap());
                i += 1;
            }
            Some(&VIRAMA) => {
                i += 1;
                // word-final virama is the short "u" sound: അത് -> athu
                if !is_malayalam_letter(chars.get(i)) {
                    out.push('u');
                }
            }
            _ => out.push('a'), // inherent vowel
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::to_manglish as m;

    #[test]
    fn common_words() {
        assert_eq!(m("നമസ്കാരം"), "namaskaaram");
        assert_eq!(m("എന്റെ"), "ente");
        assert_eq!(m("ഞങ്ങൾ"), "njangal");
        assert_eq!(m("അത്"), "athu");
        assert_eq!(m("കുട്ടി"), "kutti");
        assert_eq!(m("ഞാൻ"), "njaan");
        assert_eq!(m("മലയാളം"), "malayaalam");
        assert_eq!(m("എന്താ"), "enthaa");
    }

    #[test]
    fn english_and_mixed_pass_through() {
        assert_eq!(m("Meeting at 5 PM."), "Meeting at 5 PM.");
        assert_eq!(m("report നാളെ അയക്കാം, ok?"), "report naale ayakkaam, ok?");
    }
}
