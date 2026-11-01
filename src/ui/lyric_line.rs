//! One lyrics line, drawn as a block of sub-lines: the main line, a smaller
//! secondary line under it for the other script, and a small italic line for
//! the background vocals. This is the port of `KaraokeLineView` of the iOS
//! client.
//!
//! Each sub-line is one galley. The code builds the text of the galley
//! itself, thus it knows the character range of each timed syllable. The
//! fill moves through these ranges, syllable by syllable. All sub-lines read
//! the same syllable times, thus they fill at the same time.
//!
//! The work has two steps. [`layout`] measures the block, and [`paint`]
//! draws it in a rectangle. The caller can thus use the size (for example
//! for a fade at the edge of the view) before it allocates the space.
//!
//! A line of the second singer (`Line::opposite`) aligns to the right edge,
//! as a right-to-left line does. All its sub-lines and all their wrapped
//! rows are then flush right.

use std::sync::Arc;

use egui::text::{CCursor, LayoutJob};
use egui::{Align, Color32, FontId, Galley, Painter, Rect, Sense, pos2, vec2};

use crate::lyrics::{Line, Lyrics, Syllable};
use crate::settings::LyricsScript;

/// The secondary line and the background line are lighter than the main
/// line. The sung part keeps the full colour.
const SECONDARY_OPACITY: f32 = 0.8;

/// How far an italic glyph leans out of its box, as a part of the font
/// size. epaint moves the top of each glyph by a quarter of its height.
const ITALIC_LEAN: f32 = 0.2;

/// The text of a sub-line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    /// The text as the source gives it, for example kanji and kana.
    Original,
    /// The text in Latin letters.
    Romaji,
}

/// The place of a sub-line in the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The large line on top.
    Main,
    /// The smaller, lighter line under the main line.
    Secondary,
    /// The background vocals: a small, light, italic line at the bottom,
    /// with the times of its own syllables.
    Background,
}

/// One sub-line of the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub script: Script,
    pub role: Role,
}

/// The sub-lines of `line` for `mode`, from top to bottom. A line without
/// romaji shows only the original text in all modes. The background vocals
/// come last.
pub fn parts(line: &Line, mode: LyricsScript) -> Vec<Part> {
    let mut parts = script_parts(line, mode);
    if let Some(background) = background_part(line, mode) {
        parts.push(background);
    }
    parts
}

/// The background sub-line of `line`, if it has background vocals. When
/// `mode` shows romaji, the line uses the romaji of the syllables that have
/// it (the iOS rule).
fn background_part(line: &Line, mode: LyricsScript) -> Option<Part> {
    if line.background.iter().all(|s| s.text.trim().is_empty()) {
        return None;
    }
    let romaji = mode.shows_romaji()
        && line
            .background
            .iter()
            .any(|s| s.romanized.as_ref().is_some_and(|r| !r.trim().is_empty()));
    Some(Part {
        script: if romaji {
            Script::Romaji
        } else {
            Script::Original
        },
        role: Role::Background,
    })
}

/// The sub-lines of the main text of `line` for `mode`.
fn script_parts(line: &Line, mode: LyricsScript) -> Vec<Part> {
    let main = |script| Part {
        script,
        role: Role::Main,
    };
    let secondary = |script| Part {
        script,
        role: Role::Secondary,
    };
    if line.romanized.is_none() {
        return vec![main(Script::Original)];
    }
    match mode {
        LyricsScript::RomajiOnTop => vec![main(Script::Romaji), secondary(Script::Original)],
        LyricsScript::KanjiOnTop => vec![main(Script::Original), secondary(Script::Romaji)],
        LyricsScript::KanjiOnly => vec![main(Script::Original)],
        LyricsScript::RomajiOnly => vec![main(Script::Romaji)],
    }
}

/// True when the block aligns to the right edge: for a right-to-left line,
/// and for a line of the second singer. The second singer does not move a
/// right-to-left line to the left (the iOS rule).
pub fn aligns_right(line: &Line) -> bool {
    line.opposite || crate::bidi::is_rtl(&line.text)
}

/// One timed part of a sub-line: the characters `start..end` of its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub start: usize,
    pub end: usize,
    pub at_ms: u32,
    pub end_ms: u32,
}

impl Piece {
    fn progress(&self, position_ms: u32) -> f32 {
        crate::lyrics::Syllable {
            at_ms: self.at_ms,
            end_ms: self.end_ms,
            ..Default::default()
        }
        .progress(position_ms)
    }
}

/// The text of a sub-line and its timed pieces, in the order of the text.
/// A line without timing has no pieces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flow {
    pub text: String,
    pub pieces: Vec<Piece>,
}

impl Flow {
    /// How far into the text the song is, in characters, with a fraction
    /// for the character that the song is in.
    pub fn sung_chars(&self, position_ms: u32) -> f32 {
        let mut sung = 0.0;
        for piece in &self.pieces {
            let progress = piece.progress(position_ms);
            if progress <= 0.0 {
                break;
            }
            sung = piece.start as f32 + progress * (piece.end - piece.start) as f32;
            if progress < 1.0 {
                break;
            }
        }
        sung
    }

    /// True while a piece is in the middle of its time, thus the view must
    /// paint again for the fill to move.
    pub fn is_filling(&self, position_ms: u32) -> bool {
        self.pieces
            .iter()
            .any(|piece| position_ms >= piece.at_ms && position_ms < piece.end_ms)
    }
}

/// The text and the timed pieces of line `index` in `script`.
///
/// A karaoke source gives one piece for each syllable. The romaji pieces
/// use the times of the original syllables. A line-synced source gives only
/// the start of the line (and sometimes its end). Then the time of the line
/// is split between its words by their length. Thus the line still fills
/// word by word, as in Spicy Lyrics.
pub fn flow(lyrics: &Lyrics, index: usize, script: Script) -> Flow {
    let Some(line) = lyrics.lines.get(index) else {
        return Flow::default();
    };
    let text = match (script, &line.romanized) {
        (Script::Romaji, Some(romanized)) => romanized.as_str(),
        _ => line.text.as_str(),
    };
    // A timed line with no words is the band that plays on.
    if text.is_empty() {
        let text = if lyrics.synced { "\u{266a}" } else { "" };
        return Flow {
            text: text.into(),
            pieces: Vec::new(),
        };
    }
    let romaji = script == Script::Romaji;
    // The iOS rule: romaji syllables only when one syllable has romaji.
    // Else the romaji line fills word by word.
    let syllables_fit = !line.syllables.is_empty()
        && (!romaji || line.syllables.iter().any(|s| s.romanized.is_some()));
    if syllables_fit {
        let words = line.syllables.iter().map(|syllable| {
            let word = match &syllable.romanized {
                Some(latin) if romaji && !latin.trim().is_empty() => latin,
                _ => &syllable.text,
            };
            (word.trim(), syllable.at_ms, syllable.end_ms)
        });
        if let Some(pieces) = locate(text, words) {
            return Flow {
                text: text.into(),
                pieces,
            };
        }
        // The syllables are not in the line text in order. Build the text
        // from the syllables, thus the pieces are always correct.
        return joined(&line.syllables, romaji);
    }
    let pieces = split_line_time(lyrics, index, text)
        .and_then(|words| locate(text, words.into_iter()))
        .unwrap_or_default();
    Flow {
        text: text.into(),
        pieces,
    }
}

/// The text and the timed pieces of `part` of line `index`. The background
/// line is built from its own syllables, because it has no text field.
pub fn part_flow(lyrics: &Lyrics, index: usize, part: Part) -> Flow {
    match part.role {
        Role::Background => lyrics
            .lines
            .get(index)
            .map(|line| joined(&line.background, part.script == Script::Romaji))
            .unwrap_or_default(),
        Role::Main | Role::Secondary => flow(lyrics, index, part.script),
    }
}

/// True while a sub-line of line `index` fills.
pub fn is_filling(lyrics: &Lyrics, index: usize, mode: LyricsScript, position_ms: u32) -> bool {
    lyrics.lines.get(index).is_some_and(|line| {
        parts(line, mode)
            .into_iter()
            .any(|part| part_flow(lyrics, index, part).is_filling(position_ms))
    })
}

/// Finds each word in `text`, in order, and gives its character range.
/// `None` when a word is not in the text after the word before it.
fn locate<'a>(text: &str, words: impl Iterator<Item = (&'a str, u32, u32)>) -> Option<Vec<Piece>> {
    let mut search = 0;
    let mut pieces = Vec::new();
    for (word, at_ms, end_ms) in words {
        let start = search + text[search..].find(word)?;
        let end = start + word.len();
        pieces.push(Piece {
            start: text[..start].chars().count(),
            end: text[..end].chars().count(),
            at_ms,
            end_ms,
        });
        search = end;
    }
    Some(pieces)
}

/// A sub-line built from `syllables`. A space goes after a word, but not
/// between two Japanese or Chinese characters (the iOS rule).
fn joined(syllables: &[Syllable], romaji: bool) -> Flow {
    let mut text = String::new();
    let mut pieces = Vec::new();
    let mut chars = 0;
    let words: Vec<&str> = syllables
        .iter()
        .map(|syllable| {
            let word = match &syllable.romanized {
                Some(latin) if romaji && !latin.trim().is_empty() => latin,
                _ => &syllable.text,
            };
            word.trim()
        })
        .collect();
    for (index, (syllable, word)) in syllables.iter().zip(&words).enumerate() {
        let length = word.chars().count();
        text.push_str(word);
        pieces.push(Piece {
            start: chars,
            end: chars + length,
            at_ms: syllable.at_ms,
            end_ms: syllable.end_ms,
        });
        chars += length;
        let next = words.get(index + 1);
        if let Some(next) = next
            && !syllable.part_of_word
            && needs_space(word, next)
        {
            text.push(' ');
            chars += 1;
        }
    }
    Flow { text, pieces }
}

/// True when a space goes between `word` and `next`: always, except
/// between two Japanese or Chinese characters.
fn needs_space(word: &str, next: &str) -> bool {
    match (word.chars().next_back(), next.chars().next()) {
        (Some(last), Some(first)) => !(is_cjk(last) && is_cjk(first)),
        _ => false,
    }
}

fn is_cjk(character: char) -> bool {
    matches!(
        u32::from(character),
        0x3000..=0x30FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFF00..=0xFFEF
            | 0x2_0000..=0x2_FFFF
    )
}

/// The words of `text` with a share of the time of line `index`, for a line
/// without syllables. `None` for lyrics without timing.
fn split_line_time<'a>(
    lyrics: &Lyrics,
    index: usize,
    text: &'a str,
) -> Option<Vec<(&'a str, u32, u32)>> {
    if !lyrics.synced {
        return None;
    }
    let line = lyrics.lines.get(index)?;
    let start = line.at_ms?;
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let chars: u32 = words.iter().map(|word| word.chars().count() as u32).sum();
    let end = match line.end_ms {
        Some(end) => end,
        // Without an end, the start of the next line can come after a long
        // instrumental break. Thus the fill stops at a natural singing pace.
        None => {
            let next = lyrics
                .lines
                .get(index + 1)
                .and_then(|next| next.at_ms)
                .unwrap_or(u32::MAX);
            next.min(start.saturating_add((chars * 90).max(1_200)))
        }
    };
    if end <= start {
        return None;
    }
    // Each word weighs its letters plus one, thus short words also get time.
    let weights: Vec<u64> = words
        .iter()
        .map(|word| word.chars().count() as u64 + 1)
        .collect();
    let total: u64 = weights.iter().sum();
    let span = u64::from(end - start);
    let mut done = 0_u64;
    let mut at = start;
    Some(
        words
            .into_iter()
            .zip(weights)
            .map(|(word, weight)| {
                done += weight;
                let until = start + (span * done / total) as u32;
                let piece = (word, at, until);
                at = until;
                piece
            })
            .collect(),
    )
}

/// The fonts and the spaces of a block.
#[derive(Clone, Debug)]
pub struct Style {
    pub main: FontId,
    pub secondary: FontId,
    /// The font of the background line. The layout adds the italics.
    pub background: FontId,
    /// The space between two sub-lines.
    pub spacing: f32,
    /// The extra space above the background line.
    pub background_gap: f32,
}

/// The colours of a block. The secondary line uses them with less opacity.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// The colour of a line that does not fill now.
    pub idle: Color32,
    /// The part of the active line that the song did not reach yet.
    pub unsung: Color32,
    /// The part of the active line that the song sang.
    pub sung: Color32,
}

impl Colors {
    fn for_role(self, role: Role) -> Self {
        match role {
            Role::Main => self,
            Role::Secondary | Role::Background => Self {
                idle: self.idle.gamma_multiply(SECONDARY_OPACITY),
                unsung: self.unsung.gamma_multiply(SECONDARY_OPACITY),
                sung: self.sung,
            },
        }
    }
}

/// One measured sub-line.
struct LaidPart {
    role: Role,
    flow: Flow,
    galley: Arc<Galley>,
    /// The text reads right to left. It is on the bidi path and has no fill.
    rtl: bool,
    /// The top of the sub-line, from the top of the block.
    top: f32,
}

/// A measured block, ready to paint.
pub struct Laid {
    parts: Vec<LaidPart>,
    /// The size of the block. The width is the widest sub-line.
    pub size: egui::Vec2,
    /// The block aligns to the right edge.
    pub right: bool,
}

impl Laid {
    /// The text of the main line, for the screen readers.
    pub fn main_text(&self) -> &str {
        self.parts
            .first()
            .map_or("", |part| part.flow.text.as_str())
    }
}

/// Measures line `index` in `mode` for a column `width` points wide.
pub fn layout(
    painter: &Painter,
    lyrics: &Lyrics,
    index: usize,
    mode: LyricsScript,
    style: &Style,
    width: f32,
) -> Laid {
    let Some(line) = lyrics.lines.get(index) else {
        return Laid {
            parts: Vec::new(),
            size: egui::Vec2::ZERO,
            right: false,
        };
    };
    let right = aligns_right(line);
    let mut top = 0.0;
    let mut widest: f32 = 0.0;
    let mut laid = Vec::new();
    for (number, part) in parts(line, mode).into_iter().enumerate() {
        if number > 0 {
            top += style.spacing;
        }
        if part.role == Role::Background {
            top += style.background_gap;
        }
        let flow = part_flow(lyrics, index, part);
        let font = match part.role {
            Role::Main => style.main.clone(),
            Role::Secondary => style.secondary.clone(),
            Role::Background => style.background.clone(),
        };
        let galley = layout_part(
            painter,
            &flow.text,
            font,
            part.role == Role::Background,
            right,
            width,
        );
        let height = galley.size().y;
        widest = widest.max(galley.size().x);
        laid.push(LaidPart {
            role: part.role,
            rtl: crate::bidi::is_rtl(&flow.text),
            flow,
            galley,
            top,
        });
        top += height;
    }
    Laid {
        parts: laid,
        size: vec2(widest, top),
        right,
    }
}

/// Lays out the text of one sub-line within `width`. The colour comes at
/// paint time. Text with right-to-left letters goes through `crate::bidi`,
/// which puts the shaped runs in visual order. Other text gets `italics`
/// and, in a block on the right, rows that are flush right.
fn layout_part(
    painter: &Painter,
    text: &str,
    font: FontId,
    italics: bool,
    right: bool,
    width: f32,
) -> Arc<Galley> {
    if crate::bidi::has_rtl(text) {
        return crate::bidi::layout(painter, text, font, Color32::WHITE, width, usize::MAX, None);
    }
    let mut job = LayoutJob::simple(text.to_owned(), font, Color32::WHITE, width);
    job.wrap.break_anywhere = false;
    if right {
        job.halign = Align::RIGHT;
    }
    for section in &mut job.sections {
        section.format.italics = italics;
    }
    painter.layout_job(job)
}

/// Paints `laid` in `rect`. With `fill`, the sub-lines fill up to that
/// position of the song. Without it, they have the idle colour.
pub fn paint(painter: &Painter, laid: &Laid, rect: Rect, colors: Colors, fill: Option<u32>) {
    for part in &laid.parts {
        let colors = colors.for_role(part.role);
        let size = part.galley.size();
        let row = Rect::from_min_size(
            pos2(rect.left(), rect.top() + part.top),
            vec2(rect.width(), size.y),
        );
        // A galley with `halign` right has its anchor at the right edge.
        // A galley that `crate::bidi` aligned left, in a block on the
        // right, moves to the right edge as one block.
        let mut origin = if laid.right && part.galley.job.halign == Align::LEFT {
            pos2(row.right() - size.x, row.top())
        } else {
            crate::bidi::galley_pos(row, &part.galley)
        };
        let format = part.galley.job.sections.first().map(|s| &s.format);
        let font_size = format.map_or(16.0, |format| format.font_id.size);
        // An italic glyph leans to the right, out of the measured galley.
        // On the right, move the text back, thus it stays in the column.
        let lean = if format.is_some_and(|format| format.italics) {
            font_size * ITALIC_LEAN
        } else {
            0.0
        };
        if laid.right {
            origin.x -= lean;
        }
        let filling = fill.filter(|_| !part.rtl && !part.flow.pieces.is_empty());
        match filling {
            Some(position_ms) => {
                painter.galley_with_override_text_color(origin, part.galley.clone(), colors.unsung);
                paint_fill(
                    painter,
                    origin,
                    &part.galley,
                    part.flow.sung_chars(position_ms),
                    colors,
                    font_size * 0.6,
                    lean,
                );
            }
            None => {
                painter.galley_with_override_text_color(origin, part.galley.clone(), colors.idle);
            }
        }
    }
}

/// Allocates and paints a measured line as one widget. The widget takes
/// the full width, thus a click on the row seeks.
pub fn show(
    ui: &mut egui::Ui,
    laid: &Laid,
    sense: Sense,
    colors: Colors,
    fill: Option<u32>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), laid.size.y), sense);
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), laid, rect, colors, fill);
    }
    let text = laid.main_text().to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    response
}

/// Paints the sung part of `galley` over the unsung text: the rows that
/// the song passed in full, the current row up to the sung character, and
/// then a soft edge `feather` points wide. `lean` is how far italic glyphs
/// go out of the right side of the galley. The sung rows include it.
fn paint_fill(
    painter: &Painter,
    origin: egui::Pos2,
    galley: &Arc<Galley>,
    chars: f32,
    colors: Colors,
    feather: f32,
    lean: f32,
) {
    if chars <= 0.0 {
        return;
    }
    let total = galley.job.text.chars().count();
    let whole = chars.floor() as usize;
    if whole >= total {
        painter.galley_with_override_text_color(origin, galley.clone(), colors.sung);
        return;
    }
    let here = galley.pos_from_cursor(CCursor::new(whole));
    let next = galley.pos_from_cursor(CCursor::new(whole + 1));
    let width = if (next.min.y - here.min.y).abs() < 0.5 {
        next.min.x - here.min.x
    } else {
        feather
    };
    let edge = origin.x + here.min.x + width * chars.fract();
    let top = origin.y + here.min.y;
    let bottom = origin.y + here.max.y;
    let left = origin.x + galley.rect.min.x;
    let right = origin.x + galley.rect.max.x + lean;
    // The rows above the current row are sung in full.
    if top > origin.y + galley.rect.min.y + 0.5 {
        painter
            .with_clip_rect(Rect::from_min_max(
                pos2(left, origin.y + galley.rect.min.y),
                pos2(right, top),
            ))
            .galley_with_override_text_color(origin, galley.clone(), colors.sung);
    }
    painter
        .with_clip_rect(Rect::from_min_max(pos2(left, top), pos2(edge, bottom)))
        .galley_with_override_text_color(origin, galley.clone(), colors.sung);
    // The fill ends in a soft edge: thin slices that fade from the sung
    // colour back to the unsung colour, not a hard cut through a letter.
    const SLICES: usize = 6;
    let step = feather / SLICES as f32;
    for slice in 0..SLICES {
        let from = edge + step * slice as f32;
        let strength = 1.0 - (slice as f32 + 0.5) / SLICES as f32;
        painter
            .with_clip_rect(Rect::from_min_max(
                pos2(from, top),
                pos2((from + step).min(right), bottom),
            ))
            .galley_with_override_text_color(
                origin,
                galley.clone(),
                super::lyrics::blend(colors.unsung, colors.sung, strength),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::{Part, Role, Script, Style, aligns_right, flow, layout, part_flow, parts};
    use crate::lyrics::{Line, Lyrics, Syllable};
    use crate::settings::LyricsScript;

    fn japanese() -> Lyrics {
        let syllable = |text: &str, romaji: &str, at_ms, end_ms| Syllable {
            text: text.into(),
            at_ms,
            end_ms,
            part_of_word: true,
            romanized: Some(romaji.into()),
        };
        Lyrics {
            lines: vec![
                Line {
                    at_ms: Some(0),
                    text: "無敵の".into(),
                    end_ms: Some(3_000),
                    syllables: vec![
                        syllable("無", "mu", 0, 1_000),
                        syllable("敵", "teki", 1_000, 2_000),
                        syllable("の", "no", 2_000, 3_000),
                    ],
                    romanized: Some("muteki no".into()),
                    ..Default::default()
                },
                Line {
                    at_ms: Some(4_000),
                    text: "Hello world".into(),
                    end_ms: Some(6_000),
                    ..Default::default()
                },
            ],
            synced: true,
            instrumental: false,
            rich: true,
        }
    }

    fn part(script: Script, role: Role) -> Part {
        Part { script, role }
    }

    #[test]
    fn each_mode_puts_the_scripts_in_its_order() {
        let lyrics = japanese();
        let line = &lyrics.lines[0];
        assert_eq!(
            parts(line, LyricsScript::RomajiOnTop),
            [
                part(Script::Romaji, Role::Main),
                part(Script::Original, Role::Secondary)
            ]
        );
        assert_eq!(
            parts(line, LyricsScript::KanjiOnTop),
            [
                part(Script::Original, Role::Main),
                part(Script::Romaji, Role::Secondary)
            ]
        );
        assert_eq!(
            parts(line, LyricsScript::KanjiOnly),
            [part(Script::Original, Role::Main)]
        );
        assert_eq!(
            parts(line, LyricsScript::RomajiOnly),
            [part(Script::Romaji, Role::Main)]
        );
    }

    #[test]
    fn a_line_without_romaji_shows_only_the_original_in_all_modes() {
        let lyrics = japanese();
        let line = &lyrics.lines[1];
        for mode in LyricsScript::ALL {
            assert_eq!(parts(line, mode), [part(Script::Original, Role::Main)]);
            let shown = flow(&lyrics, 1, parts(line, mode)[0].script);
            assert_eq!(shown.text, "Hello world");
        }
    }

    #[test]
    fn romaji_fills_with_the_times_of_the_original_syllables() {
        let lyrics = japanese();
        let original = flow(&lyrics, 0, Script::Original);
        let romaji = flow(&lyrics, 0, Script::Romaji);
        assert_eq!(original.text, "無敵の");
        assert_eq!(romaji.text, "muteki no");
        // "teki" is the characters 2..6 of the romaji, with the time of 敵.
        assert_eq!((romaji.pieces[1].start, romaji.pieces[1].end), (2, 6));
        assert_eq!(romaji.pieces[1].at_ms, original.pieces[1].at_ms);
        assert_eq!(romaji.pieces[1].end_ms, original.pieces[1].end_ms);
        // Half of 敵: one and a half kanji, and "mu" plus half of "teki".
        assert!((original.sung_chars(1_500) - 1.5).abs() < 0.01);
        assert!((romaji.sung_chars(1_500) - 4.0).abs() < 0.01);
        // "mu" and "teki" sung, half of "no": the space is skipped.
        assert!((romaji.sung_chars(2_500) - 8.0).abs() < 0.01);
        assert_eq!(romaji.sung_chars(9_000), 9.0);
        assert!(romaji.is_filling(2_500));
        assert!(!romaji.is_filling(3_000));
    }

    #[test]
    fn syllables_out_of_order_build_their_own_text() {
        let mut lyrics = japanese();
        lyrics.lines[0].romanized = Some("something else".into());
        let romaji = flow(&lyrics, 0, Script::Romaji);
        // The syllables are one word, thus no spaces.
        assert_eq!(romaji.text, "mutekino");
        assert_eq!((romaji.pieces[2].start, romaji.pieces[2].end), (6, 8));
        // Japanese words without `part_of_word` get no space between them,
        // Latin words get one.
        for syllable in &mut lyrics.lines[0].syllables {
            syllable.part_of_word = false;
        }
        lyrics.lines[0].text = "x".into();
        assert_eq!(flow(&lyrics, 0, Script::Original).text, "無敵の");
        assert_eq!(flow(&lyrics, 0, Script::Romaji).text, "mu teki no");
    }

    #[test]
    fn a_line_synced_line_is_split_into_words_that_fill_in_turn() {
        let lyrics = Lyrics {
            lines: vec![
                Line {
                    at_ms: Some(10_000),
                    text: "She's into superstitions".into(),
                    end_ms: Some(12_000),
                    ..Default::default()
                },
                Line {
                    at_ms: Some(13_000),
                    text: "Black cats".into(),
                    ..Default::default()
                },
            ],
            synced: true,
            instrumental: false,
            rich: false,
        };
        let line = flow(&lyrics, 0, Script::Original);
        let pieces = &line.pieces;
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].at_ms, 10_000);
        assert_eq!(pieces[2].end_ms, 12_000);
        // The words follow one another without gaps.
        assert_eq!(pieces[0].end_ms, pieces[1].at_ms);
        assert_eq!(pieces[1].end_ms, pieces[2].at_ms);
        // The longest word gets the most time.
        assert!(pieces[2].end_ms - pieces[2].at_ms > pieces[0].end_ms - pieces[0].at_ms);
        // "She's" is five characters: at its half, 2.5 characters are sung.
        let half = (pieces[0].at_ms + pieces[0].end_ms) / 2;
        assert!((line.sung_chars(half) - 2.5).abs() < 0.05);
        // Without an end, the last line stops at a singing pace.
        let last = flow(&lyrics, 1, Script::Original);
        assert!(last.pieces.last().unwrap().end_ms <= 13_000 + 1_200);
        // Plain lyrics have no time to fill through.
        let plain = Lyrics {
            synced: false,
            ..lyrics.clone()
        };
        assert!(flow(&plain, 0, Script::Original).pieces.is_empty());
        assert_eq!(flow(&plain, 0, Script::Original).text, lyrics.lines[0].text);
    }

    #[test]
    fn a_timed_line_without_words_is_a_note() {
        let lyrics = Lyrics {
            lines: vec![Line {
                at_ms: Some(0),
                ..Default::default()
            }],
            synced: true,
            ..Default::default()
        };
        let note = flow(&lyrics, 0, Script::Original);
        assert_eq!(note.text, "\u{266a}");
        assert!(note.pieces.is_empty());
    }

    /// A duet line with background vocals: "oh yeah" in two syllables,
    /// with the romaji "o" for the first one only.
    fn duet() -> Lyrics {
        let syllable = |text: &str, romaji: Option<&str>, at_ms, end_ms| Syllable {
            text: text.into(),
            at_ms,
            end_ms,
            part_of_word: false,
            romanized: romaji.map(Into::into),
        };
        let mut lyrics = japanese();
        lyrics.lines[1].opposite = true;
        lyrics.lines[1].background = vec![
            syllable("oh", Some("o"), 4_500, 5_000),
            syllable("yeah", None, 5_000, 6_500),
        ];
        lyrics
    }

    #[test]
    fn the_second_singer_and_right_to_left_lines_align_right() {
        let lyrics = duet();
        assert!(!aligns_right(&lyrics.lines[0]));
        assert!(aligns_right(&lyrics.lines[1]));
        let hebrew = Line {
            text: "שלום עולם".into(),
            ..Default::default()
        };
        assert!(aligns_right(&hebrew));
        // The second singer does not move a right-to-left line to the left.
        let hebrew_duet = Line {
            opposite: true,
            ..hebrew
        };
        assert!(aligns_right(&hebrew_duet));
    }

    #[test]
    fn background_vocals_come_last_with_their_own_script() {
        let lyrics = duet();
        let line = &lyrics.lines[1];
        assert_eq!(
            parts(line, LyricsScript::KanjiOnly),
            [
                part(Script::Original, Role::Main),
                part(Script::Original, Role::Background)
            ]
        );
        // A mode with romaji uses the romaji of the background syllables.
        for mode in [
            LyricsScript::KanjiOnTop,
            LyricsScript::RomajiOnTop,
            LyricsScript::RomajiOnly,
        ] {
            assert_eq!(
                parts(line, mode).last(),
                Some(&part(Script::Romaji, Role::Background))
            );
        }
        // A Japanese line keeps its two scripts above the background line.
        let mut lyrics = lyrics;
        lyrics.lines[0].background = lyrics.lines[1].background.clone();
        assert_eq!(
            parts(&lyrics.lines[0], LyricsScript::KanjiOnTop),
            [
                part(Script::Original, Role::Main),
                part(Script::Romaji, Role::Secondary),
                part(Script::Romaji, Role::Background)
            ]
        );
        // Background syllables without words give no sub-line.
        lyrics.lines[0].background = vec![Syllable {
            text: " ".into(),
            ..Default::default()
        }];
        assert_eq!(parts(&lyrics.lines[0], LyricsScript::KanjiOnly).len(), 1);
    }

    #[test]
    fn background_vocals_fill_with_their_own_times() {
        let lyrics = duet();
        let original = part_flow(&lyrics, 1, part(Script::Original, Role::Background));
        assert_eq!(original.text, "oh yeah");
        assert_eq!((original.pieces[1].start, original.pieces[1].end), (3, 7));
        assert_eq!(original.pieces[0].at_ms, 4_500);
        assert_eq!(original.pieces[1].end_ms, 6_500);
        // The lead sings alone before 4.5 s: the background is not sung.
        assert_eq!(original.sung_chars(4_400), 0.0);
        assert!((original.sung_chars(5_750) - 5.0).abs() < 0.01);
        assert!(original.is_filling(6_000));
        // The romaji replaces only the syllable that has romaji.
        let romaji = part_flow(&lyrics, 1, part(Script::Romaji, Role::Background));
        assert_eq!(romaji.text, "o yeah");
        // The main flows stay as they were.
        assert_eq!(
            part_flow(&lyrics, 1, part(Script::Original, Role::Main)),
            flow(&lyrics, 1, Script::Original)
        );
    }

    #[test]
    fn a_right_block_has_rows_flush_right_and_an_italic_background() {
        let lyrics = Lyrics {
            lines: vec![Line {
                at_ms: Some(0),
                text: "one two three four five six seven eight nine ten".into(),
                end_ms: Some(4_000),
                opposite: true,
                background: vec![Syllable {
                    text: "yeah".into(),
                    at_ms: 1_000,
                    end_ms: 2_000,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            synced: true,
            ..Default::default()
        };
        let style = Style {
            main: egui::FontId::proportional(20.0),
            secondary: egui::FontId::proportional(14.0),
            background: egui::FontId::proportional(14.0),
            spacing: 4.0,
            background_gap: 1.5,
        };
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter();
            let laid = layout(painter, &lyrics, 0, LyricsScript::KanjiOnly, &style, 120.0);
            assert!(laid.right);
            let main = &laid.parts[0].galley;
            assert!(main.rows.len() > 1, "the test line must wrap");
            assert_eq!(main.job.halign, egui::Align::RIGHT);
            // Each wrapped row ends at the anchor, the right edge.
            for row in &main.rows {
                assert!(row.rect().max.x.abs() < 1.0, "{:?}", row.rect());
            }
            let background = &laid.parts[1];
            assert_eq!(background.role, Role::Background);
            assert!(background.galley.job.sections[0].format.italics);
            assert!(!main.job.sections[0].format.italics);
            // The background starts below the main line and the two gaps.
            assert!((background.top - (main.size().y + 5.5)).abs() < 0.01);
            // A line of the lead singer stays on the left.
            let mut lead = lyrics.clone();
            lead.lines[0].opposite = false;
            let laid = layout(painter, &lead, 0, LyricsScript::KanjiOnly, &style, 120.0);
            assert!(!laid.right);
            assert_eq!(laid.parts[0].galley.job.halign, egui::Align::LEFT);
        });
        // The test draws no frame, thus it drops the font texture.
        output.textures_delta.clear();
    }

    /// An italic glyph leans out of the right side of its galley. A row
    /// that the song passed must get the sung colour up to the end of its
    /// last glyph, not only to the edge of the galley.
    #[test]
    fn a_sung_italic_row_fills_its_leaning_last_glyph() {
        let word = |text: &str, at_ms| Syllable {
            text: text.into(),
            at_ms,
            end_ms: at_ms + 1_000,
            ..Default::default()
        };
        let lyrics = Lyrics {
            lines: vec![Line {
                at_ms: Some(0),
                text: "x".into(),
                end_ms: Some(6_000),
                background: ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"]
                    .iter()
                    .zip((0..).step_by(1_000))
                    .map(|(text, at_ms)| word(text, at_ms))
                    .collect(),
                ..Default::default()
            }],
            synced: true,
            ..Default::default()
        };
        let style = Style {
            main: egui::FontId::proportional(20.0),
            secondary: egui::FontId::proportional(14.0),
            background: egui::FontId::proportional(14.0),
            spacing: 4.0,
            background_gap: 1.5,
        };
        let colors = super::Colors {
            idle: egui::Color32::GRAY,
            unsung: egui::Color32::DARK_GRAY,
            sung: egui::Color32::WHITE,
        };
        let ctx = egui::Context::default();
        let mut sung_right = 0.0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter();
            let laid = layout(painter, &lyrics, 0, LyricsScript::KanjiOnly, &style, 120.0);
            let background = &laid.parts[1].galley;
            assert!(background.rows.len() > 1, "the background must wrap");
            sung_right = background.rect.max.x + 14.0 * super::ITALIC_LEAN;
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, laid.size);
            // Half of "foxtrot", on the last row: the rows above are sung.
            super::paint(painter, &laid, rect, colors, Some(5_500));
        });
        output.textures_delta.clear();
        let widest_fill = output
            .shapes
            .iter()
            .map(|shape| shape.clip_rect.max.x)
            .filter(|right| *right < 1_000.0)
            .fold(0.0_f32, f32::max);
        assert!(
            widest_fill >= sung_right - 0.01,
            "{widest_fill} < {sung_right}"
        );
    }
}
