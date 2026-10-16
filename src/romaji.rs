//! Romaji for Japanese lyrics that arrive without one.
//!
//! Spicy Lyrics romanizes many Japanese songs itself, but not all of them, and
//! Spotify and LRCLIB never do. For those lines, lindera splits the text into
//! words with the embedded IPADIC dictionary, which also gives each word's
//! reading in katakana, and wana_kana writes the reading in Latin letters.
//!
//! Every character of the original line is mapped to where its romaji starts,
//! so the syllables of a karaoke line can take their own share of the
//! romanized line and still light up one by one.

use std::borrow::Cow;
use std::sync::OnceLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use wana_kana::ConvertJapanese as _;

use crate::lyrics::Lyrics;

/// The segmenter, loaded on first use. Loading the dictionary takes a moment,
/// so it happens once, on the backend runtime, never on the UI thread.
fn segmenter() -> Option<&'static Segmenter> {
    static SEGMENTER: OnceLock<Option<Segmenter>> = OnceLock::new();
    SEGMENTER
        .get_or_init(|| match load_dictionary("embedded://ipadic") {
            Ok(dictionary) => Some(Segmenter::new(Mode::Normal, dictionary, None)),
            Err(error) => {
                log::warn!("romaji dictionary unavailable: {error}");
                None
            }
        })
        .as_ref()
}

/// Hiragana, katakana and their half-width and phonetic extensions.
pub fn is_kana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{309f}' | '\u{30a0}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{ff66}'..='\u{ff9f}')
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '々' | '〆' | 'ヶ')
}

fn is_japanese(c: char) -> bool {
    is_kana(c) || is_kanji(c)
}

/// A romanized line, and for each character of the original, the character
/// position in `text` where its romaji starts. `starts` has one more entry
/// than the original has characters: the end.
#[derive(Debug, PartialEq, Eq)]
pub struct Romanized {
    pub text: String,
    pub starts: Vec<usize>,
}

impl Romanized {
    /// The romaji for the original characters `from..to`, trimmed.
    pub fn slice(&self, from: usize, to: usize) -> String {
        let from = self.starts.get(from).copied().unwrap_or(0);
        let to = self.starts.get(to).copied().unwrap_or(from).max(from);
        self.text
            .chars()
            .skip(from)
            .take(to - from)
            .collect::<String>()
            .trim()
            .to_string()
    }
}

/// What a token is, for deciding where the spaces go.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Japanese,
    Latin,
    /// Punctuation that leans on the word before it: , . ! ? ) and so on.
    Closing,
    /// Punctuation that leans on the word after it: ( and opening quotes.
    Opening,
}

/// Japanese and full-width punctuation in the ASCII form romaji lyrics use.
fn ascii_punctuation(c: char) -> Option<&'static str> {
    Some(match c {
        '、' | '，' => ",",
        '。' | '．' => ".",
        '！' => "!",
        '？' => "?",
        '「' | '『' | '“' => "\"",
        '」' | '』' | '”' => "\"",
        '（' => "(",
        '）' => ")",
        '・' => "-",
        '〜' | '～' => "~",
        '…' => "...",
        '：' => ":",
        '；' => ";",
        '　' => " ",
        _ => return None,
    })
}

/// A non-Japanese surface, with full-width letters and Japanese punctuation
/// turned into their ASCII forms.
fn plain(surface: &str) -> String {
    let mut out = String::with_capacity(surface.len());
    for c in surface.chars() {
        if let Some(ascii) = ascii_punctuation(c) {
            out.push_str(ascii);
        } else if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
            out.push(char::from_u32(c as u32 - 0xfee0).unwrap_or(c));
        } else {
            out.push(c);
        }
    }
    out
}

fn kind_of(text: &str) -> Kind {
    if text.chars().any(is_japanese) {
        return Kind::Japanese;
    }
    if text.chars().any(char::is_alphanumeric) {
        return Kind::Latin;
    }
    if text.trim_start().starts_with(['(', '"', '[', '{']) {
        Kind::Opening
    } else {
        Kind::Closing
    }
}

/// The romaji of one token, from its IPADIC details: part of speech at 0,
/// its sub-class at 1, and the reading in katakana at 7. The flag is true
/// when the token ends in a small tsu, which doubles the next word's first
/// consonant: 言っ + た is "itta".
fn token_romaji(surface: &str, details: &[String]) -> (String, bool) {
    if !surface.chars().any(is_japanese) {
        return (plain(surface), false);
    }
    let pos = details.first().map(String::as_str).unwrap_or("");
    // The particles are written as they sound.
    if pos == "助詞" {
        match surface {
            "は" => return ("wa".into(), false),
            "へ" => return ("e".into(), false),
            "を" => return ("wo".into(), false),
            _ => {}
        }
    }
    let reading = details
        .get(7)
        .map(String::as_str)
        .filter(|reading| *reading != "*" && reading.chars().all(|c| is_kana(c) || c == 'ー'));
    let kana = match reading {
        Some(reading) => reading.to_string(),
        // An unknown word: its kana can still be read; kanji stay as they are.
        None => surface.to_string(),
    };
    let doubles = kana.ends_with(['っ', 'ッ']);
    let kana = kana.trim_end_matches(['っ', 'ッ']);
    // Punctuation first: wana_kana has its own idea of what ・ becomes.
    (plain(kana).to_romaji(), doubles)
}

/// Where each mora of `romaji` ends, in characters: after a vowel, after an
/// "n" that no vowel or "y" follows, and around anything that is not a letter.
fn mora_ends(romaji: &str) -> Vec<usize> {
    let chars: Vec<char> = romaji.chars().collect();
    let vowel = |c: char| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u');
    let mut ends = Vec::new();
    for (index, &c) in chars.iter().enumerate() {
        let next = chars.get(index + 1).copied();
        let end = vowel(c)
            || !c.is_ascii_alphabetic()
            || (c.eq_ignore_ascii_case(&'n')
                && next.is_none_or(|next| !vowel(next) && next != 'y'))
            || next.is_some_and(|next| !next.is_ascii_alphabetic());
        if end {
            ends.push(index + 1);
        }
    }
    if ends.last() != Some(&chars.len()) {
        ends.push(chars.len());
    }
    ends
}

/// Whether a token joins the word before it with no space: endings, helper
/// verbs and suffixes belong to their word, as in "shitai" and "kite".
fn attaches(surface: &str, details: &[String], kind: Kind, previous: Option<Kind>) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    if kind == Kind::Closing || previous == Kind::Opening {
        return true;
    }
    if kind != Kind::Japanese || previous != Kind::Japanese {
        return false;
    }
    let pos = details.first().map(String::as_str).unwrap_or("");
    let sub = details.get(1).map(String::as_str).unwrap_or("");
    pos == "助動詞"
        || sub == "接尾"
        || (pos == "動詞" && sub == "非自立")
        || (pos == "助詞" && sub == "接続助詞" && matches!(surface, "て" | "で"))
        || surface == "ー"
}

/// The romaji of a line, or `None` for a line with no Japanese in it.
pub fn romanize(line: &str) -> Option<Romanized> {
    if !line.chars().any(is_japanese) {
        return None;
    }
    let segmenter = segmenter()?;
    let mut tokens = segmenter.segment(Cow::Borrowed(line)).ok()?;
    // The character index of every byte offset that starts a character.
    let mut char_at = vec![0_usize; line.len() + 1];
    let mut count = 0;
    for (index, (byte, _)) in line.char_indices().enumerate() {
        char_at[byte] = index;
        count = index + 1;
    }
    char_at[line.len()] = count;
    let mut starts = vec![0_usize; count + 1];
    let mut text = String::new();
    let mut length = 0_usize;
    let mut space = false;
    let mut previous: Option<Kind> = None;
    let mut doubling = false;
    let mut done_byte = 0;
    for token in tokens.iter_mut() {
        // Text the segmenter left out between tokens is white space.
        if token.byte_start > done_byte {
            if line[done_byte..token.byte_start]
                .chars()
                .any(char::is_whitespace)
            {
                space = true;
            }
            starts[char_at[done_byte]..char_at[token.byte_start]].fill(length);
        }
        done_byte = token.byte_end;
        let first = char_at[token.byte_start];
        let last = char_at[token.byte_end];
        let surface = token.surface.to_string();
        if surface.trim().is_empty() {
            space = true;
            starts[first..last].fill(length);
            continue;
        }
        let details: Vec<String> = token
            .details()
            .iter()
            .map(|detail| (*detail).to_string())
            .collect();
        let (mut romaji, doubles) = token_romaji(&surface, &details);
        let kind = kind_of(&surface);
        // A small tsu before this word doubles its first consonant, and the
        // two halves are one word: "kassaratte", not "ka sara tte".
        let mut joined = false;
        if doubling
            && let Some(first_letter) = romaji.chars().next()
            && first_letter.is_ascii_alphabetic()
            && !matches!(first_letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'n')
        {
            romaji.insert(
                0,
                if romaji.starts_with("ch") {
                    't'
                } else {
                    first_letter
                },
            );
            joined = true;
        }
        doubling = doubles;
        if !text.is_empty() && !joined && (space || !attaches(&surface, &details, kind, previous)) {
            text.push(' ');
            length += 1;
        }
        space = false;
        // Characters are given whole morae, so a syllable split inside a
        // word falls between sounds: 修羅|場 is "shura|ba".
        let ends = mora_ends(&romaji);
        let span = (last - first).max(1);
        for (offset, index) in (first..last).enumerate() {
            let mora = offset * ends.len() / span;
            starts[index] = length + if mora == 0 { 0 } else { ends[mora - 1] };
        }
        let width = romaji.chars().count();
        text.push_str(&romaji);
        length += width;
        previous = Some(kind);
    }
    starts[char_at[done_byte]..count].fill(length);
    starts[count] = length;
    Some(Romanized { text, starts })
}

/// The character range each syllable covers in `text`, found in order.
fn syllable_ranges(
    text: &str,
    syllables: &[crate::lyrics::Syllable],
) -> Option<Vec<(usize, usize)>> {
    let mut search = 0;
    let mut ranges = Vec::with_capacity(syllables.len());
    for syllable in syllables {
        let word = syllable.text.trim();
        let start = search + text[search..].find(word)?;
        let end = start + word.len();
        ranges.push((text[..start].chars().count(), text[..end].chars().count()));
        search = end;
    }
    Some(ranges)
}

/// Gives every Japanese line that lacks one a romanization, and its
/// syllables their share of it. Only songs with kana are touched, so Chinese
/// lyrics, which share the kanji but not their readings, are left alone.
pub fn fill(lyrics: &mut Lyrics) {
    if !lyrics
        .lines
        .iter()
        .any(|line| line.text.chars().any(is_kana))
    {
        return;
    }
    for line in &mut lyrics.lines {
        if line.romanized.is_some() {
            continue;
        }
        let Some(romanized) = romanize(&line.text) else {
            continue;
        };
        if romanized.text.is_empty() || romanized.text == line.text {
            continue;
        }
        if !line.syllables.is_empty() {
            let Some(ranges) = syllable_ranges(&line.text, &line.syllables) else {
                // Without the syllables' places the line is still shown in
                // romaji, filling word by word.
                line.romanized = Some(romanized.text);
                continue;
            };
            for (syllable, (from, to)) in line.syllables.iter_mut().zip(ranges) {
                syllable.romanized = Some(romanized.slice(from, to));
            }
        }
        line.romanized = Some(romanized.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::{Line, Syllable};

    #[test]
    fn kanji_kana_and_particles_become_spaced_romaji() {
        assert_eq!(romanize("無敵の笑顔で").unwrap().text, "muteki no egao de");
        assert_eq!(
            romanize("私は君を待つ").unwrap().text,
            "watashi wa kimi wo matsu"
        );
    }

    #[test]
    fn a_small_tsu_doubles_the_next_consonant() {
        assert_eq!(
            romanize("ねえ、言ったよね？").unwrap().text,
            "nee, itta yo ne?"
        );
        assert_eq!(romanize("息を吸って").unwrap().text, "iki wo sutte");
    }

    #[test]
    fn syllables_split_between_sounds() {
        let romanized = romanize("修羅場").unwrap();
        assert_eq!(romanized.text, "shuraba");
        assert_eq!(romanized.slice(0, 2), "shura");
        assert_eq!(romanized.slice(2, 3), "ba");
    }

    #[test]
    fn english_words_and_punctuation_keep_their_place() {
        let text = romanize("ドキドキしてRouge?").unwrap().text;
        assert!(text.starts_with("dokidoki"), "{text}");
        assert!(text.ends_with("Rouge?"), "{text}");
        assert!(romanize("Feuer! Sperrfeuer! Los!").is_none());
    }

    #[test]
    fn every_character_maps_into_the_romaji_in_order() {
        let romanized = romanize("君を待つ").unwrap();
        assert_eq!(romanized.starts.len(), 5);
        assert!(romanized.starts.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(
            *romanized.starts.last().unwrap(),
            romanized.text.chars().count()
        );
    }

    #[test]
    fn syllables_get_their_share_of_the_romanized_line() {
        let syllable = |text: &str| Syllable {
            text: text.into(),
            part_of_word: true,
            ..Default::default()
        };
        let mut lyrics = Lyrics {
            lines: vec![Line {
                at_ms: Some(0),
                text: "君を待つ".into(),
                syllables: vec![
                    syllable("君"),
                    syllable("を"),
                    syllable("待"),
                    syllable("つ"),
                ],
                ..Default::default()
            }],
            synced: true,
            rich: true,
            instrumental: false,
        };
        fill(&mut lyrics);
        let line = &lyrics.lines[0];
        assert_eq!(line.romanized.as_deref(), Some("kimi wo matsu"));
        let parts: Vec<&str> = line
            .syllables
            .iter()
            .map(|syllable| syllable.romanized.as_deref().unwrap())
            .collect();
        assert_eq!(parts[0], "kimi");
        assert_eq!(parts[1], "wo");
        assert_eq!(format!("{}{}", parts[2], parts[3]), "matsu");
    }

    #[test]
    fn songs_without_kana_are_left_alone() {
        let mut lyrics = Lyrics {
            lines: vec![Line {
                text: "月亮代表我的心".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        fill(&mut lyrics);
        assert!(lyrics.lines[0].romanized.is_none());
    }
}
