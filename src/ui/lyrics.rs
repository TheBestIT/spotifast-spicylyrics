//! The words of the playing track, in a side panel that follows the song.

use egui::{Align, Color32, Frame, Layout, Margin, Rect, Sense, UiBuilder, pos2, vec2};

use std::borrow::Cow;
use std::sync::Arc;

use egui::text::CCursor;

use crate::app::App;
use crate::i18n::{gettext, pgettext};
use crate::lyrics::{Lyrics, Syllable};
use crate::model::{Action, Loadable};
use crate::theme::{self, Icon};

use super::widgets;

const LINE_SIZE: f32 = 19.0;
const LINE_GAP: f32 = 10.0;
/// Where the line being sung sits, as a fraction of the visible lyrics from
/// the top: high up, so the lines to come fill most of the view.
const SUNG_LINE_AT: f32 = 0.2;

/// Scrolls so the middle of `line` sits `SUNG_LINE_AT` of the way down the
/// visible lyrics.
fn show_sung_line(ui: &egui::Ui, line: Rect, animation: Option<egui::style::ScrollAnimation>) {
    let above = (ui.clip_rect().height() * SUNG_LINE_AT - line.height() / 2.0).max(0.0);
    let target = Rect::from_min_max(pos2(line.left(), line.top() - above), line.max);
    match animation {
        Some(animation) => ui.scroll_to_rect_animation(target, Some(Align::Min), animation),
        None => ui.scroll_to_rect(target, Some(Align::Min)),
    }
}
/// How long a line takes to light up or fade.
const LIGHT_UP_SECONDS: f32 = 0.22;

fn blend(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from(egui::Rgba::from(from) * (1.0 - t) + egui::Rgba::from(to) * t)
}

/// Whether any piece is in the middle of being sung, so the view must keep
/// repainting for the fill to move.
fn is_filling(pieces: &[Syllable], position_ms: u32) -> bool {
    pieces
        .iter()
        .any(|piece| position_ms >= piece.at_ms && position_ms < piece.end_ms)
}

/// The timed pieces that line `index` fills through. A karaoke source gives
/// one per syllable. A line-synced source gives only the line's start (and
/// sometimes its end), so the line's time is split between its words by
/// their length, and the line still fills word by word, as in Spicy Lyrics.
/// With `romanize`, the pieces are the romanized syllables, or the romanized
/// words when only the line has a romanization.
fn timed_pieces(lyrics: &Lyrics, index: usize, romanize: bool) -> Option<Cow<'_, [Syllable]>> {
    let line = lyrics.lines.get(index)?;
    let romanized = romanize && line.romanized.is_some();
    if !line.syllables.is_empty() {
        if !romanized {
            return Some(Cow::Borrowed(line.syllables.as_slice()));
        }
        let pieces: Option<Vec<Syllable>> = line
            .syllables
            .iter()
            .map(|syllable| {
                Some(Syllable {
                    text: syllable.romanized.clone()?,
                    ..syllable.clone()
                })
            })
            .collect();
        if let Some(pieces) = pieces {
            return Some(Cow::Owned(pieces));
        }
        // Some syllables lack a romanization: split the romanized line's
        // time between its words instead.
    }
    if !lyrics.synced {
        return None;
    }
    let start = line.at_ms?;
    let words: Vec<&str> = line.display(romanize).split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let chars: u32 = words.iter().map(|word| word.chars().count() as u32).sum();
    let end = match line.end_ms {
        Some(end) => end,
        // Without an end, the next line's start may sit after a long
        // instrumental break, so the fill stops at a natural singing pace.
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
    // Each word weighs its letters plus one, so short words still get time.
    let weights: Vec<u64> = words
        .iter()
        .map(|word| word.chars().count() as u64 + 1)
        .collect();
    let total: u64 = weights.iter().sum();
    let span = u64::from(end - start);
    let mut done = 0_u64;
    let mut at = start;
    let pieces = words
        .iter()
        .zip(weights)
        .map(|(word, weight)| {
            done += weight;
            let until = start + (span * done / total) as u32;
            let piece = Syllable {
                text: (*word).to_string(),
                at_ms: at,
                end_ms: until,
                ..Default::default()
            };
            at = until;
            piece
        })
        .collect();
    Some(Cow::Owned(pieces))
}

/// How far into `text` the song is, in characters, with a fraction for the
/// character being sung. Each timed piece is found in the text in order, so
/// the position follows the same words the line shows.
fn sung_chars(text: &str, pieces: &[Syllable], position_ms: u32) -> f32 {
    let mut search = 0;
    let mut sung = 0.0;
    for piece in pieces {
        let word = piece.text.trim();
        let Some(found) = text[search..].find(word) else {
            break;
        };
        let start = search + found;
        let end = start + word.len();
        let progress = piece.progress(position_ms);
        if progress > 0.0 {
            let before = text[..start].chars().count() as f32;
            let length = text[start..end].chars().count() as f32;
            sung = before + progress * length;
        }
        if progress < 1.0 {
            break;
        }
        search = end;
    }
    sung
}

/// Draws the active line as one laid-out galley, so it wraps and measures
/// exactly as it does when it is not active, then lights its text up to where
/// the song is: the rows already sung in full, the current row up to the sung
/// character, and a soft edge after it. `galley` is laid out in `base`.
#[allow(clippy::too_many_arguments)]
fn karaoke_label(
    ui: &mut egui::Ui,
    galley: Arc<egui::Galley>,
    pieces: &[Syllable],
    position_ms: u32,
    base: Color32,
    sung: Color32,
    feather: f32,
    sense: Sense,
) -> egui::Response {
    let response = ui.add(egui::Label::new(galley.clone()).sense(sense));
    let chars = sung_chars(&galley.job.text, pieces, position_ms);
    if chars <= 0.0 {
        return response;
    }
    // The label places the galley so that its own rectangle lands on the
    // response; cursor positions are relative to the galley's origin.
    let origin = response.rect.min - galley.rect.min.to_vec2();
    let painter = ui.painter();
    let total = galley.job.text.chars().count();
    let whole = chars.floor() as usize;
    if whole >= total {
        painter.galley_with_override_text_color(origin, galley, sung);
        return response;
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
    let right = origin.x + galley.rect.max.x;
    // The rows above the one being sung are sung in full.
    if top > origin.y + galley.rect.min.y + 0.5 {
        painter
            .with_clip_rect(Rect::from_min_max(
                pos2(origin.x + galley.rect.min.x, origin.y + galley.rect.min.y),
                pos2(right, top),
            ))
            .galley_with_override_text_color(origin, galley.clone(), sung);
    }
    painter
        .with_clip_rect(Rect::from_min_max(
            pos2(origin.x + galley.rect.min.x, top),
            pos2(edge, bottom),
        ))
        .galley_with_override_text_color(origin, galley.clone(), sung);
    // The fill ends in a soft edge: thin slices that fade from the sung
    // colour back to the quiet one, instead of a hard cut through a letter.
    const SLICES: usize = 6;
    let step = feather / SLICES as f32;
    for slice in 0..SLICES {
        let left = edge + step * slice as f32;
        let strength = 1.0 - (slice as f32 + 0.5) / SLICES as f32;
        painter
            .with_clip_rect(Rect::from_min_max(
                pos2(left, top),
                pos2((left + step).min(right), bottom),
            ))
            .galley_with_override_text_color(origin, galley.clone(), blend(base, sung, strength));
    }
    response
}

/// Opens the Spicy Lyrics key setup. Accented while no key works yet, as a
/// quiet invitation.
fn spicy_button(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette, idle: Color32) {
    use crate::model::SpicyKeyStatus;
    if app.spicy_key_status == SpicyKeyStatus::Unknown {
        app.actions.push(Action::CheckSpicyKey);
    }
    let invite = matches!(
        app.spicy_key_status,
        SpicyKeyStatus::Missing | SpicyKeyStatus::Rejected
    );
    let color = if invite { palette.accent } else { idle };
    if theme::icon_button(
        ui,
        Icon::Sparkles,
        18.0,
        color,
        palette.text,
        &gettext(app.locale, "Spicy Lyrics: word-by-word lyrics"),
    )
    .clicked()
    {
        app.actions.push(Action::OpenSpicyLyricsSetup);
    }
}

/// The Romaji switch, shown only while the lyrics have a romanization. It is
/// filled in the accent colour while the romanized text is shown.
fn romanize_button(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette) {
    let offered =
        matches!(&app.lyrics, Loadable::Loaded(Some(lyrics)) if lyrics.has_romanization());
    if offered
        && theme::pill_button(
            ui,
            palette,
            &pgettext(app.locale, "lyrics", "Romaji"),
            app.settings.lyrics_romanize,
        )
        .on_hover_text(gettext(app.locale, "Show the lyrics in Latin letters"))
        .clicked()
    {
        app.actions.push(Action::ToggleLyricsRomanize);
    }
}

pub fn side_panel(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let fit = super::yielding_panel(
        ui.ctx(),
        "lyrics-panel",
        theme::SIDE_PANEL_MIN_WIDTH..=640.0,
        app.settings.lyrics_width,
        ui.available_width() - super::topbar::least_width(ui.ctx()),
    );
    let panel = egui::Panel::right("lyrics-panel")
        .resizable(true)
        .default_size(app.settings.lyrics_width)
        .size_range(fit.range.clone())
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.panel)
                .inner_margin(Margin::symmetric(12, 12)),
        );
    let response = panel.show(ui, |ui| {
        let window_controls = super::window_controls_reservation(
            ui.ctx(),
            app.show_queue_panel,
            app.show_lyrics_panel,
            ui.available_width(),
        );
        ui.add_space(window_controls.lyrics_top);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            theme::text(
                ui,
                gettext(app.locale, "Lyrics"),
                theme::bold(18.0),
                palette.text,
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(
                    ui,
                    Icon::X,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Close"),
                )
                .clicked()
                {
                    app.actions.push(Action::ToggleLyricsPanel);
                }
                if theme::icon_button(
                    ui,
                    Icon::Expand,
                    18.0,
                    palette.secondary,
                    palette.text,
                    &gettext(app.locale, "Full screen lyrics"),
                )
                .clicked()
                {
                    app.actions.push(Action::SetLyricsFullscreen(true));
                }
                spicy_button(app, ui, &palette, palette.secondary);
                romanize_button(app, ui, &palette);
                let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
                if loaded
                    && !app.lyrics_following
                    && theme::pill_button(
                        ui,
                        &palette,
                        &pgettext(app.locale, "lyrics", "Follow"),
                        false,
                    )
                    .clicked()
                {
                    app.lyrics_following = true;
                    app.lyrics_line_shown = None;
                }
            });
        });
        ui.add_space(8.0);
        contents(app, ui);
    });
    let current_width = response.response.rect.width();
    if (app.settings.lyrics_width - current_width).abs() > 1.0
        && super::panel_width_chosen(ui.ctx(), "lyrics-panel", &fit)
    {
        app.settings.lyrics_width = current_width;
        app.actions.push(Action::SettingsChanged);
    }
}

fn contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            if !app.network_reachable {
                widgets::empty_state(
                    ui,
                    &palette,
                    Icon::WifiOff,
                    &gettext(app.locale, "You're offline"),
                    &gettext(
                        app.locale,
                        "Spotifast will reconnect automatically when your connection returns.",
                    ),
                );
            } else {
                widgets::loading_row(ui, &palette, app.locale);
            }
            return;
        }
        Loadable::Failed(error) => {
            if !app.network_reachable {
                widgets::empty_state(
                    ui,
                    &palette,
                    Icon::WifiOff,
                    &gettext(app.locale, "You're offline"),
                    &gettext(
                        app.locale,
                        "Spotifast will reconnect automatically when your connection returns.",
                    ),
                );
                return;
            }
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.secondary);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.request_lyrics();
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    let romanize = app.settings.lyrics_romanize;
    let follow = app.lyrics_following && app.lyrics_line_shown != Some(active);
    // The line being sung is bold and in the accent colour; every other
    // line is quiet, regular text, the same before and after it has been
    // sung. A line takes 220 ms to light up or fade, as in omarchy-lyrics.
    let quiet = palette.text.gamma_multiply(0.45);
    let scroll = crate::autoscroll::show(
        ui,
        egui::ScrollArea::vertical()
            .id_salt("lyrics-scroll")
            .auto_shrink([false, false]),
        egui::Vec2b::new(false, true),
        |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                );
            }
            ui.add_space(12.0);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(index),
                    is_active,
                    crate::motion::seconds(ui.ctx(), LIGHT_UP_SECONDS),
                );
                let color = blend(quiet, palette.accent, lit);
                let font = if lit > 0.5 {
                    theme::bold(LINE_SIZE)
                } else {
                    theme::regular(LINE_SIZE)
                };
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.display(romanize)
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                // The active line lights up word by word; every other line
                // keeps the quiet, line-level look.
                let pieces = if is_active && !crate::bidi::is_rtl(text) {
                    timed_pieces(&lyrics, index, romanize)
                } else {
                    None
                };
                let response = if let Some(pieces) = &pieces {
                    let galley = ui.painter().layout(
                        text.to_string(),
                        theme::bold(LINE_SIZE),
                        quiet,
                        ui.available_width(),
                    );
                    karaoke_label(
                        ui,
                        galley,
                        pieces,
                        now.position_ms,
                        quiet,
                        palette.accent,
                        LINE_SIZE * 0.6,
                        sense,
                    )
                } else if crate::bidi::is_rtl(text) {
                    let galley = crate::bidi::layout(
                        ui.painter(),
                        text,
                        font,
                        color,
                        ui.available_width(),
                        usize::MAX,
                        None,
                    );
                    ui.add(egui::Label::new(galley).sense(sense))
                } else {
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).font(font).color(color))
                            .sense(sense),
                    )
                };
                crate::autoscroll::row(ui, &response);
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.lyrics_following = true;
                }
                if is_active && follow {
                    show_sung_line(ui, rect, Some(crate::motion::scroll(ui.ctx(), 0.45)));
                }
                ui.add_space(LINE_GAP);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if app.lyrics_following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                );
            }
            // Room for the last line to rise to where a sung line sits.
            ui.add_space((ui.clip_rect().height() * (1.0 - SUNG_LINE_AT)).max(60.0));
        },
    );
    crate::autoscroll::lyrics(ui, scroll.id);
    // The word-by-word fill moves between frames, so keep painting while a
    // word is in progress.
    if now.playing
        && active.is_some_and(|index| {
            timed_pieces(&lyrics, index, romanize)
                .is_some_and(|pieces| is_filling(&pieces, now.position_ms))
        })
    {
        ui.ctx().request_repaint();
    }
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if ui.rect_contains_pointer(scroll.inner_rect)
        && ui.input(|input| input.smooth_scroll_delta.y != 0.0)
    {
        app.lyrics_following = false;
    }
    app.lyrics_line_shown = Some(active);
}

pub fn fullscreen(app: &mut App, ui: &mut egui::Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::Palette::dark().window))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            background(app, ui, rect);
            let top = theme::titlebar_inset(ui.ctx()) + 24.0;
            if app.now_playing().is_some() && rect.width() >= COVER_BESIDE_MIN_WIDTH {
                with_cover(app, ui, rect, top);
                return;
            }
            let width = fullscreen_content_width(rect.width());
            let region = Rect::from_min_max(
                pos2(rect.center().x - width / 2.0, rect.top() + top),
                pos2(rect.center().x + width / 2.0, rect.bottom()),
            );
            let mut content = ui.new_child(UiBuilder::new().max_rect(region));
            fullscreen_header(app, &mut content);
            content.add_space(20.0);
            track_heading(app, &mut content);
            content.add_space(16.0);
            fullscreen_contents(app, &mut content);
        });
}

/// The widest the lyrics get beside the cover, so lines stay easy to read.
const LYRICS_BESIDE_WIDTH: f32 = 640.0;

/// The narrowest window that shows the cover beside the lyrics; narrower
/// ones keep a single column with a small cover in the heading.
const COVER_BESIDE_MIN_WIDTH: f32 = 900.0;

/// Full screen with the cover large: beside the lyrics when there are
/// words to follow, and alone in the middle when there are none, a calm
/// view of what is playing.
fn with_cover(app: &mut App, ui: &mut egui::Ui, rect: Rect, top: f32) {
    let outer = Rect::from_min_max(
        pos2(rect.left() + 48.0, rect.top() + top),
        pos2(rect.right() - 48.0, rect.bottom() - 40.0),
    );
    let mut header = ui.new_child(UiBuilder::new().max_rect(outer));
    fullscreen_header(app, &mut header);
    let below = Rect::from_min_max(
        pos2(outer.left(), header.min_rect().bottom() + 24.0),
        outer.max,
    );
    // The cover moves aside only for words to read. While they load it
    // stays in the middle, so a song that turns out to have none never
    // moves at all.
    let words = matches!(&app.lyrics, Loadable::Loaded(Some(lyrics)) if !lyrics.instrumental);
    // The queue docks right while its flag is on, in both arrangements.
    // Without it the room below is untouched, preserving the stock layout.
    let gap = 64.0;
    let queue_width = if app.show_queue_panel {
        // Shared with the side panel's persisted width; the column's drag
        // strip below writes the same field.
        app.settings.queue_width.clamp(
            super::queue::FULLSCREEN_COLUMN_MIN,
            super::queue::FULLSCREEN_COLUMN_MAX,
        )
    } else {
        0.0
    };
    let queue_gap = if app.show_queue_panel { gap } else { 0.0 };
    let lyrics_end = below.right() - queue_width - queue_gap;
    if app.show_queue_panel {
        let queue_rect = Rect::from_min_max(
            pos2(below.right() - queue_width, below.top()),
            pos2(below.right(), below.bottom()),
        );
        let mut queue_ui = ui.new_child(UiBuilder::new().max_rect(queue_rect));
        super::queue::fullscreen_column(app, &mut queue_ui);
    }
    if words {
        if app.show_queue_panel {
            // The cover docks left; the lyrics take the middle that remains.
            let room = lyrics_end - below.left();
            let side = (room * 0.42).min(below.height() - 90.0).clamp(160.0, 520.0);
            let column = Rect::from_min_size(
                pos2(below.left(), below.center().y - (side + 90.0) / 2.0),
                vec2(side, side + 90.0),
            );
            big_cover(app, ui, column, Align::Min);
            let lyrics_avail = lyrics_end - column.right() - gap;
            let lyrics_width = lyrics_avail.clamp(80.0, LYRICS_BESIDE_WIDTH);
            // Centre the block between the cover and the queue when there is
            // room to spare; it hugs the cover on narrow windows.
            let lyrics_x = column.right() + gap + (lyrics_avail - lyrics_width).max(0.0) / 2.0;
            let lyrics = Rect::from_min_max(
                pos2(lyrics_x, below.top()),
                pos2(lyrics_x + lyrics_width, below.bottom()),
            );
            let mut content = ui.new_child(UiBuilder::new().max_rect(lyrics));
            fullscreen_contents(app, &mut content);
        } else {
            // The cover and the lyrics are one group, centred in the window.
            let side = (below.width() * 0.38)
                .min(below.height() - 90.0)
                .clamp(200.0, 520.0);
            let lyrics_width = (below.width() - side - gap).min(LYRICS_BESIDE_WIDTH);
            let left = below.center().x - (side + gap + lyrics_width) / 2.0;
            let column = Rect::from_min_size(
                pos2(left, below.center().y - (side + 90.0) / 2.0),
                vec2(side, side + 90.0),
            );
            big_cover(app, ui, column, Align::Min);
            let lyrics = Rect::from_min_max(
                pos2(column.right() + gap, below.top()),
                pos2(column.right() + gap + lyrics_width, below.bottom()),
            );
            let mut content = ui.new_child(UiBuilder::new().max_rect(lyrics));
            fullscreen_contents(app, &mut content);
        }
    } else {
        let span = Rect::from_min_max(
            pos2(below.left(), below.top()),
            pos2(lyrics_end, below.bottom()),
        );
        // Without the queue the span is the whole room, matching the stock
        // centred cover exactly.
        let min_side = if app.show_queue_panel { 160.0 } else { 200.0 };
        let side = (span.height() - 140.0)
            .min(span.width() * 0.5)
            .clamp(min_side, 560.0);
        let column = Rect::from_center_size(span.center(), vec2(side, side + 90.0));
        big_cover(app, ui, column, Align::Center);
        // Why there are no words, quietly, under the song, or that they
        // are still being fetched.
        let (heading, detail) = match &app.lyrics {
            Loadable::Loaded(Some(_)) => (
                gettext(app.locale, "Instrumental"),
                gettext(app.locale, "No timed lyrics for this track."),
            ),
            Loadable::Loaded(None) => (
                gettext(app.locale, "No lyrics"),
                gettext(app.locale, "No lyrics found for this track."),
            ),
            Loadable::Failed(error) => (
                gettext(
                    app.locale,
                    // Translators: Keep {error} unchanged. It is the original failure detail.
                    "Couldn't fetch the lyrics: {error}",
                )
                .replace("{error}", error)
                .into(),
                Default::default(),
            ),
            Loadable::NotLoaded | Loadable::Loading => {
                (gettext(app.locale, "Loading…"), Default::default())
            }
        };
        let heading = ui.painter().text(
            pos2(column.center().x, column.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            heading,
            theme::semibold(13.0),
            Color32::from_gray(200),
        );
        if matches!(app.lyrics, Loadable::Failed(_)) {
            let retry = Rect::from_center_size(
                pos2(column.center().x, heading.bottom() + 24.0),
                vec2(column.width(), 32.0),
            );
            let mut retry_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(retry)
                    .layout(Layout::top_down(Align::Center)),
            );
            let label = gettext(app.locale, "Try again");
            if theme::pill_button(&mut retry_ui, &theme::Palette::dark(), &label, false).clicked() {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        ui.painter().text(
            pos2(column.center().x, heading.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            detail,
            theme::regular(13.0),
            Color32::from_gray(170),
        );
    }
}

/// The playing song's cover filling the top of `column`, with its title and
/// artists beneath, aligned to its left edge or centred.
fn big_cover(app: &App, ui: &mut egui::Ui, column: Rect, align: Align) {
    let Some(now) = app.now_playing() else {
        return;
    };
    let side = column.width();
    let cover = Rect::from_min_size(column.min, vec2(side, side));
    let radius = 10.0;
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(140),
        }
        .as_shape(cover, radius),
    );
    widgets::playing_cover(
        ui,
        &theme::Palette::dark(),
        app.backend.art(),
        "fullscreen-lyrics",
        now.art_url.as_deref().or(now.art_small.as_deref()),
        cover,
        radius,
    );
    let words = Rect::from_min_max(pos2(column.left(), cover.bottom() + 18.0), column.max);
    let mut text = ui.new_child(
        UiBuilder::new()
            .max_rect(words)
            .layout(Layout::top_down(align)),
    );
    text.spacing_mut().item_spacing.y = 4.0;
    text.add(
        egui::Label::new(
            egui::RichText::new(&now.title)
                .font(theme::semibold(22.0))
                .color(Color32::WHITE),
        )
        .truncate(),
    );
    text.add(
        egui::Label::new(
            egui::RichText::new(&now.subtitle)
                .font(theme::regular(14.0))
                .color(Color32::from_gray(225)),
        )
        .truncate(),
    );
}

fn fullscreen_content_width(viewport_width: f32) -> f32 {
    let available = (viewport_width - 48.0).max(0.0);
    (viewport_width * 0.72).clamp(400.0, 960.0).min(available)
}

fn preferred_backdrop_art(small: Option<String>, large: Option<String>) -> Option<String> {
    small.or(large)
}

fn background(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    theme::apply_local(ui, &theme::Palette::dark());
    let art = app
        .now_playing()
        .and_then(|now| preferred_backdrop_art(now.art_small, now.art_url));
    let painter = ui.painter().with_clip_rect(rect);
    if let Some(texture) = app
        .lyrics_backdrop
        .texture(ui.ctx(), app.backend.art(), art.as_deref())
    {
        painter.image(
            texture.id(),
            rect,
            cover_uv(rect.size(), texture.size_vec2()),
            Color32::from_gray(180),
        );
    }
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(120));
    widgets::paint_vertical_gradient(
        ui,
        rect,
        Color32::from_black_alpha(0),
        Color32::from_black_alpha(95),
    );
}

fn cover_uv(view: egui::Vec2, image: egui::Vec2) -> Rect {
    let ratio = (view.x / view.y.max(1.0)) / (image.x / image.y.max(1.0));
    let size = if ratio > 1.0 {
        vec2(1.0, 1.0 / ratio)
    } else {
        vec2(ratio, 1.0)
    };
    Rect::from_center_size(pos2(0.5, 0.5), size)
}

fn fullscreen_header(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    ui.horizontal(|ui| {
        theme::text(
            ui,
            gettext(app.locale, "Lyrics"),
            theme::bold(18.0),
            palette.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(
                ui,
                Icon::Shrink,
                18.0,
                palette.text,
                palette.text,
                &gettext(app.locale, "Leave full screen (Esc)"),
            )
            .clicked()
            {
                app.actions.push(Action::SetLyricsFullscreen(false));
            }
            spicy_button(app, ui, &palette, palette.text);
            romanize_button(app, ui, &palette);
            let loaded = matches!(&app.lyrics, Loadable::Loaded(Some(_)));
            if loaded
                && !app.lyrics_following
                && theme::pill_button(
                    ui,
                    &palette,
                    &pgettext(app.locale, "lyrics", "Follow"),
                    false,
                )
                .clicked()
            {
                app.actions.push(Action::FollowLyrics);
            }
        });
    });
}

fn track_heading(app: &App, ui: &mut egui::Ui) {
    if let Some(now) = app.now_playing() {
        ui.horizontal(|ui| {
            let size = 52.0;
            let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
            widgets::playing_cover(
                ui,
                &theme::Palette::dark(),
                app.backend.art(),
                "lyrics-heading",
                app.small_art(&now, size),
                rect,
                4.0,
            );
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.title)
                            .font(theme::semibold(22.0))
                            .color(Color32::WHITE),
                    )
                    .truncate(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&now.subtitle)
                            .font(theme::regular(13.0))
                            .color(Color32::from_gray(235)),
                    )
                    .truncate(),
                );
            });
        });
    }
}

fn fullscreen_contents(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::Palette::dark();
    let Some(now) = app.now_playing() else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Mic,
            &gettext(app.locale, "Nothing playing"),
            &gettext(app.locale, "Play a song to see its lyrics."),
        );
        return;
    };
    let lyrics = match &app.lyrics {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading_row(ui, &palette, app.locale);
            return;
        }
        Loadable::Failed(error) => {
            let message = gettext(
                app.locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error);
            ui.add_space(8.0);
            theme::text(ui, message, theme::regular(13.0), palette.text);
            ui.add_space(8.0);
            if theme::pill_button(ui, &palette, &gettext(app.locale, "Try again"), false).clicked()
            {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
        Loadable::Loaded(None) => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Mic,
                &gettext(app.locale, "No lyrics"),
                &gettext(app.locale, "No lyrics found for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            widgets::empty_state(
                ui,
                &palette,
                Icon::Music,
                &gettext(app.locale, "Instrumental"),
                &gettext(app.locale, "No timed lyrics for this track."),
            );
            return;
        }
        Loadable::Loaded(Some(lyrics)) => lyrics.clone(),
    };

    let active = lyrics.active_line(now.position_ms);
    let romanize = app.settings.lyrics_romanize;
    // A Follow click resets the remembered line after drawing. Other frames
    // record the shown line before any line-click action restores following.
    if !app
        .actions
        .iter()
        .any(|action| matches!(action, Action::FollowLyrics))
    {
        app.actions.push(Action::LyricsLineShown(active));
    }
    let viewport = ui.available_rect_before_wrap();
    let manual_scroll = ui.rect_contains_pointer(viewport)
        && ui.input(|input| {
            input.smooth_scroll_delta.y != 0.0
                || (input.pointer.primary_down() && input.pointer.delta().y != 0.0)
        });
    let following = app.lyrics_following && !manual_scroll;
    let follow = following && app.lyrics_line_shown != Some(active);
    let animation = crate::motion::scroll(ui.ctx(), 0.45);
    let size = (ui.available_width() * 0.046).clamp(28.0, 42.0);
    // The line being sung brightens; all lines keep the same font metrics
    // so highlighting cannot rewrap the words during a transition.
    // A line takes 300 ms to light up or fade.
    let quiet = palette.text.gamma_multiply(0.68);
    ui.spacing_mut().scroll.fade.strength = 0.0;
    egui::ScrollArea::vertical()
        .id_salt(("fullscreen-lyrics-scroll", &now.uri))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Before the first line there is nothing to highlight, so the
            // panel sits at the top rather than wherever it was left.
            if follow && lyrics.synced && active.is_none() {
                let top = ui.cursor().min;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_size(top, egui::vec2(1.0, 1.0)),
                    Some(Align::Min),
                    animation,
                );
            }
            // Room for the first line to sit where a sung line sits, and for
            // the last to rise to it.
            let (padding, below) = if lyrics.synced {
                (
                    (viewport.height() * SUNG_LINE_AT - size).max(12.0),
                    viewport.height() * (1.0 - SUNG_LINE_AT),
                )
            } else {
                (12.0, 60.0)
            };
            ui.add_space(padding);
            for (index, line) in lyrics.lines.iter().enumerate() {
                let is_active = active == Some(index);
                let lit = ui.ctx().animate_bool_with_time(
                    egui::Id::new("lyric-line").with(("fullscreen", &now.uri, index)),
                    is_active,
                    crate::motion::seconds(ui.ctx(), 0.3),
                );
                let color = if lyrics.synced {
                    blend(quiet, palette.text, lit)
                } else {
                    palette.text
                };
                let font = theme::bold(size);
                // A timed line with no words is the band playing on.
                let text = if line.text.is_empty() && lyrics.synced {
                    "\u{266a}"
                } else {
                    line.display(romanize)
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let galley = crate::bidi::layout(
                    ui.painter(),
                    text,
                    font,
                    color,
                    ui.available_width(),
                    usize::MAX,
                    None,
                );
                let center = ui.cursor().top() + galley.size().y * 0.5;
                let edge = ((center - viewport.top()).min(viewport.bottom() - center)
                    / (size * 2.0))
                    .clamp(0.0, 1.0);
                // The active line fills word by word.
                let pieces = if is_active && lyrics.synced && !crate::bidi::is_rtl(text) {
                    timed_pieces(&lyrics, index, romanize)
                } else {
                    None
                };
                let response = ui
                    .scope(|ui| {
                        ui.multiply_opacity(edge * edge * (3.0 - 2.0 * edge));
                        if let Some(pieces) = &pieces {
                            // The same layout as the line's own galley, in the
                            // quiet colour, so the line does not move or rewrap.
                            let quiet_galley = crate::bidi::layout(
                                ui.painter(),
                                text,
                                theme::bold(size),
                                quiet,
                                ui.available_width(),
                                usize::MAX,
                                None,
                            );
                            karaoke_label(
                                ui,
                                quiet_galley,
                                pieces,
                                now.position_ms,
                                quiet,
                                palette.text,
                                size * 0.6,
                                sense,
                            )
                        } else {
                            ui.add(egui::Label::new(galley).sense(sense))
                        }
                    })
                    .inner;
                let rect = response.rect;
                if lyrics.synced
                    && response.clicked()
                    && let Some(at_ms) = line.at_ms
                {
                    app.actions.push(Action::Seek(at_ms));
                    app.actions.push(Action::FollowLyrics);
                }
                if is_active && follow {
                    show_sung_line(ui, rect, Some(animation));
                }
                ui.add_space(27.0);
            }
            // Words without timing can only be followed by the clock: sit
            // at the part of the text the song is probably at.
            if following && !lyrics.synced && now.duration_ms > 0 {
                let fraction =
                    (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
                let content = ui.min_rect();
                let y = content.top() + content.height() * fraction as f32;
                ui.scroll_to_rect_animation(
                    egui::Rect::from_min_max(
                        egui::pos2(content.left(), y),
                        egui::pos2(content.right(), y + 1.0),
                    ),
                    Some(Align::Center),
                    animation,
                );
            }
            ui.add_space(below.max(60.0));
        });
    // Scrolling by hand means the reader wants to look elsewhere; the
    // Follow button in the header picks the song back up.
    if manual_scroll && app.lyrics_following {
        app.actions.push(Action::PauseLyricsFollow);
    }
    // The active line fills continuously, so repaint every frame while a word
    // is in progress; otherwise wake just in time for the next line.
    if now.playing
        && active.is_some_and(|index| {
            timed_pieces(&lyrics, index, romanize)
                .is_some_and(|pieces| is_filling(&pieces, now.position_ms))
        })
    {
        ui.ctx().request_repaint();
    } else if now.playing
        && lyrics.synced
        && let Some(next) = lyrics
            .lines
            .iter()
            .filter_map(|line| line.at_ms)
            .find(|at| *at > now.position_ms)
    {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(u64::from(
                next - now.position_ms,
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::{fullscreen_content_width, preferred_backdrop_art, sung_chars, timed_pieces};
    use crate::lyrics::{Line, Lyrics, Syllable};

    #[test]
    fn the_sung_position_moves_through_the_text_word_by_word() {
        let piece = |text: &str, at_ms, end_ms| Syllable {
            text: text.into(),
            at_ms,
            end_ms,
            ..Default::default()
        };
        let pieces = [piece("She's", 0, 1_000), piece("into", 1_000, 2_000)];
        let text = "She's into";
        assert_eq!(sung_chars(text, &pieces, 0), 0.0);
        // Half of "She's" is five characters, half sung.
        assert!((sung_chars(text, &pieces, 500) - 2.5).abs() < 0.01);
        // "She's" sung, the space skipped, half of "into".
        assert!((sung_chars(text, &pieces, 1_500) - 8.0).abs() < 0.01);
        assert_eq!(sung_chars(text, &pieces, 9_000), 10.0);
    }

    #[test]
    fn romanized_syllables_fill_through_the_romanized_line() {
        let syllable = |text: &str, romaji: &str, at_ms, end_ms| Syllable {
            text: text.into(),
            at_ms,
            end_ms,
            part_of_word: true,
            romanized: Some(romaji.into()),
        };
        let lyrics = Lyrics {
            lines: vec![Line {
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
            }],
            synced: true,
            instrumental: false,
            rich: true,
        };
        let original = timed_pieces(&lyrics, 0, false).unwrap();
        assert_eq!(original[1].text, "敵");
        let romaji = timed_pieces(&lyrics, 0, true).unwrap();
        assert_eq!(romaji[1].text, "teki");
        let text = lyrics.lines[0].display(true);
        assert_eq!(text, "muteki no");
        // "mu" and "teki" sung, half of "no": the space is skipped.
        assert!((sung_chars(text, &romaji, 2_500) - 8.0).abs() < 0.01);
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
        let pieces = timed_pieces(&lyrics, 0, false).expect("a timed line fills");
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].at_ms, 10_000);
        assert_eq!(pieces[2].end_ms, 12_000);
        // The words follow one another without gaps.
        assert_eq!(pieces[0].end_ms, pieces[1].at_ms);
        assert_eq!(pieces[1].end_ms, pieces[2].at_ms);
        // The longest word gets the most time.
        assert!(pieces[2].end_ms - pieces[2].at_ms > pieces[0].end_ms - pieces[0].at_ms);
        // Without an end, the last line stops at a singing pace.
        let last = timed_pieces(&lyrics, 1, false).expect("fills");
        assert!(last.last().unwrap().end_ms <= 13_000 + 1_200);
        // Plain lyrics carry no time to fill through.
        let plain = Lyrics {
            synced: false,
            ..lyrics.clone()
        };
        assert!(timed_pieces(&plain, 0, false).is_none());
    }

    #[test]
    fn fullscreen_backdrop_prefers_small_art_with_large_art_as_fallback() {
        let small = "small".to_string();
        let large = "large".to_string();
        assert_eq!(
            preferred_backdrop_art(Some(small.clone()), Some(large.clone())),
            Some(small)
        );
        assert_eq!(
            preferred_backdrop_art(None, Some(large.clone())),
            Some(large)
        );
        assert_eq!(preferred_backdrop_art(None, None), None);
    }

    #[test]
    fn fullscreen_content_width_never_inverts_a_narrow_viewport() {
        for viewport_width in [0.0, 24.0, 47.0, 48.0, 64.0, 760.0, 2_000.0] {
            let width = fullscreen_content_width(viewport_width);
            assert!(width >= 0.0);
            assert!(width <= (viewport_width - 48.0).max(0.0));
        }
        assert_eq!(fullscreen_content_width(47.0), 0.0);
        assert_eq!(fullscreen_content_width(2_000.0), 960.0);
    }
}
