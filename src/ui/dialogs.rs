//! Modal dialogs: playlist details, confirmations, shortcuts.

use egui::{Align, CornerRadius, Frame, Layout, Margin, Stroke};

use crate::app::App;
use crate::i18n::{Locale, gettext, ngettext};
use crate::model::{Action, Dialog};
use crate::theme;

pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(dialog) = app.dialog.clone() else {
        return;
    };
    let palette = app.palette;
    let locale = app.locale;
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(Margin::same(24))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: palette.shadow,
        });
    let response = egui::Modal::new(egui::Id::new("dialog"))
        .frame(frame)
        .backdrop_color(egui::Color32::from_black_alpha(if palette.dark {
            150
        } else {
            80
        }))
        .show(ctx, |ui| {
            ui.set_width(420.0);
            match dialog {
                Dialog::PersonalAppIntro => {
                    theme::text(ui, gettext(locale, "Spend less time waiting for Spotify"), theme::bold(20.0), palette.text);
                    ui.add_space(12.0);
                    for text in [
                        gettext(locale, "Spotifast's default connection shares Spotify's request limit with other listeners. When it gets busy, loading music and using playback controls can take longer."),
                        gettext(locale, "Your Premium account lets you create a free personal Spotify app. Connect it here to give supported requests your own allowance. Some pages still use the shared connection."),
                        gettext(locale, "Setup takes a few minutes. You can also find it later in Settings under Personal Spotify app."),
                    ] {
                        ui.add(egui::Label::new(egui::RichText::new(text).font(theme::regular(14.0)).color(palette.secondary)).wrap());
                        ui.add_space(10.0);
                    }
                    ui.add_space(8.0);
                    buttons(ui, |ui| {
                        if theme::pill_button(ui, &palette, &gettext(locale, "Set up personal app"), true).clicked() {
                            app.actions.push(Action::OpenPersonalAppSetup);
                        }
                        if theme::pill_button(ui, &palette, &gettext(locale, "Keep shared app"), false).clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                    });
                }
                Dialog::SpicyLyrics { .. } => spicy_lyrics(app, ui),
                Dialog::CreatePlaylist { .. } => create_playlist(app, ui),
                Dialog::EditPlaylist { .. } => edit_playlist(app, ui),
                Dialog::ConfirmDeletePlaylist { id, name, owned } => {
                    theme::text(
                        ui,
                        if owned {
                            gettext(locale, "Delete playlist?")
                        } else {
                            gettext(locale, "Remove from Your Library?")
                        },
                        theme::bold(20.0),
                        palette.text,
                    );
                    ui.add_space(8.0);
                    let body = if owned {
                        // Translators: {name} is a playlist name.
                        gettext(locale, "Delete “{name}”? You can recover it from Spotify for 90 days.")
                    } else {
                        // Translators: {name} is a playlist name.
                        gettext(locale, "“{name}” will no longer appear in Your Library.")
                    }
                    .replace("{name}", &name);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(body)
                                .font(theme::regular(14.0))
                                .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    ui.add_space(20.0);
                    buttons(ui, |ui| {
                        if theme::pill_button(
                            ui,
                            &palette,
                            &if owned {
                                gettext(locale, "Delete")
                            } else {
                                gettext(locale, "Remove")
                            },
                            true,
                        )
                        .clicked()
                        {
                            app.actions.push(Action::DeletePlaylist(id.clone()));
                        }
                        if theme::pill_button(ui, &palette, &gettext(locale, "Cancel"), false).clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                    });
                }
                Dialog::ConfirmPlaylistDuplicates {
                    playlist_id,
                    playlist_name,
                    items,
                    position,
                    duplicate_uris,
                } => {
                    let multiple = items.len() > 1;
                    theme::text(
                        ui,
                        if multiple {
                            gettext(locale, "Songs already in this playlist")
                        } else {
                            gettext(locale, "Song already in this playlist")
                        },
                        theme::bold(20.0),
                        palette.text,
                    );
                    ui.add_space(8.0);
                    let body = duplicate_message(locale, &playlist_name, &items, &duplicate_uris);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(body)
                                .font(theme::regular(14.0))
                                .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    ui.add_space(20.0);
                    buttons(ui, |ui| {
                        if theme::pill_button(ui, &palette, &gettext(locale, "Add anyway"), true).clicked() {
                            app.actions.push(Action::ConfirmAddToPlaylist {
                                playlist_id: playlist_id.clone(),
                                playlist_name: playlist_name.clone(),
                                items: items.clone(),
                                position,
                            });
                        }
                        if theme::pill_button(ui, &palette, &gettext(locale, "Cancel"), false).clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                    });
                }
                Dialog::Shortcuts => {
                    theme::text(ui, gettext(locale, "Keyboard shortcuts"), theme::bold(20.0), palette.text);
                    ui.add_space(12.0);
                    // `theme::text` truncates, which in a grid makes each cell
                    // claim almost no width and turns "Ctrl+Shift+A" into
                    // "Ctrl…". A shortcut is unusable when abbreviated, so
                    // these cells are sized to their content.
                    let cell = |ui: &mut egui::Ui, text: &str, font: egui::FontId, color| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).font(font).color(color))
                                .extend()
                                .selectable(false),
                        );
                    };
                    // The list is longer than a small screen is tall, so it
                    // scrolls inside the dialog rather than running off the
                    // bottom with the Done button beyond reach.
                    let room = ui.ctx().content_rect().height() - 190.0;
                    crate::autoscroll::show(
                        ui,
                        egui::ScrollArea::vertical()
                        .max_height(room.max(120.0))
                        .auto_shrink([false, true]),
                        egui::Vec2b::new(false, true),
                        |ui| {
                            egui::Grid::new("shortcuts")
                                .num_columns(2)
                                .spacing([24.0, 8.0])
                                .show(ui, |ui| {
                                    for (keys, description) in super::keys::shortcuts(locale) {
                                        cell(ui, &keys, theme::semibold(13.0), palette.text);
                                        cell(
                                            ui,
                                            &description,
                                            theme::regular(13.5),
                                            palette.secondary,
                                        );
                                        ui.end_row();
                                    }
                                });
                        },
                    );
                    ui.add_space(16.0);
                    buttons(ui, |ui| {
                        if theme::pill_button(ui, &palette, &gettext(locale, "Done"), true).clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                    });
                }
                Dialog::PremiumNeeded => {
                    theme::text(
                        ui,
                        gettext(locale, "This account cannot play music here"),
                        theme::bold(20.0),
                        palette.text,
                    );
                    ui.add_space(8.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(gettext(
                                locale,
                                "Playback needs Spotify Premium. Free accounts can browse and search, but cannot play music through Spotifast.",
                            ))
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    ui.add_space(20.0);
                    buttons(ui, |ui| {
                        if theme::pill_button(ui, &palette, &gettext(locale, "OK"), true).clicked() {
                            app.actions.push(Action::CloseDialog);
                        }
                    });
                }
            }
        });
    app.dialog_rect = Some(response.response.rect);
    if response.should_close() {
        app.actions.push(Action::CloseDialog);
    }
}

// Keep button rows content-sized when the shared modal area last held a taller dialog.
fn buttons(ui: &mut egui::Ui, add_buttons: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
        Layout::right_to_left(Align::Center),
        add_buttons,
    );
}

fn duplicate_message(
    locale: Locale,
    playlist_name: &str,
    items: &[crate::api::models::PlayableItem],
    duplicate_uris: &[String],
) -> String {
    let mut seen = std::collections::HashSet::new();
    let names: Vec<&str> = items
        .iter()
        .filter(|item| duplicate_uris.iter().any(|uri| uri == item.uri()))
        .map(crate::api::models::PlayableItem::name)
        .filter(|name| seen.insert(*name))
        .collect();
    let one_item = items.len() == 1;
    let message = match names.as_slice() {
        [] if one_item => {
            // Translators: {playlist} is a playlist name.
            gettext(
                locale,
                "This song is already in “{playlist}”. Add it again?",
            )
        }
        [] => {
            // Translators: {playlist} is a playlist name.
            gettext(
                locale,
                "This song is already in “{playlist}”. Add them anyway?",
            )
        }
        [name] if one_item => {
            // Translators: {name} is a song name and {playlist} a playlist name.
            gettext(locale, "“{name}” is already in “{playlist}”. Add it again?")
                .replace("{name}", name)
                .into()
        }
        [name] => {
            // Translators: {name} is a song name and {playlist} a playlist name.
            gettext(
                locale,
                "“{name}” is already in “{playlist}”. Add them anyway?",
            )
            .replace("{name}", name)
            .into()
        }
        [first, second] => {
            // Translators: {first} and {second} are song names, {playlist} a playlist name.
            gettext(
                locale,
                "“{first}” and “{second}” are already in “{playlist}”. Add them anyway?",
            )
            .replace("{first}", first)
            .replace("{second}", second)
            .into()
        }
        [first, second, rest @ ..] => ngettext(
            locale,
            // Translators: {first} and {second} are song names, {count} is how many more songs are already in the playlist, and {playlist} is a playlist name.
            "“{first}”, “{second}”, and {count} more are already in “{playlist}”. Add them anyway?",
            "“{first}”, “{second}”, and {count} more are already in “{playlist}”. Add them anyway?",
            rest.len() as u32,
        )
        .replace("{first}", first)
        .replace("{second}", second)
        .replace("{count}", &rest.len().to_string())
        .into(),
    };
    message.replace("{playlist}", playlist_name)
}

fn text_field(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    locale: crate::i18n::Locale,
    id: &str,
    text: &mut String,
    hint: &str,
    focus: bool,
) -> egui::Response {
    let response = Frame::new()
        .fill(palette.surface)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            super::widgets::text_edit(
                ui,
                locale,
                egui::TextEdit::singleline(text)
                    .id(egui::Id::new(id))
                    .hint_text(egui::RichText::new(hint).color(palette.dim))
                    .font(theme::regular(14.0))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner;
    if focus && ui.memory(|memory| memory.focused().is_none()) {
        response.request_focus();
    }
    response
}

/// Where a Spicy Lyrics key is made.
pub const SPICY_DASHBOARD_URL: &str = "https://developers.spicylyrics.org/dashboard/applications";
/// How Spicy Lyrics keys work, native apps included.
pub const SPICY_KEYS_URL: &str = "https://developers.spicylyrics.org/docs/keys";
/// The terms a key holder accepts.
pub const SPICY_TERMS_URL: &str = "https://developers.spicylyrics.org/terms";

/// A short line about the key's state, with an icon in a matching colour.
pub fn spicy_status(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    locale: Locale,
    status: &crate::model::SpicyKeyStatus,
) {
    use crate::model::SpicyKeyStatus;
    let (icon, color, text) = match status {
        SpicyKeyStatus::Unknown | SpicyKeyStatus::Checking => {
            (None, palette.secondary, gettext(locale, "Checking the key…").to_string())
        }
        SpicyKeyStatus::Missing => (
            Some(theme::Icon::Info),
            palette.secondary,
            gettext(locale, "No key yet. Lyrics come from Spotify and LRCLIB, line by line.")
                .to_string(),
        ),
        SpicyKeyStatus::Working => (
            Some(theme::Icon::CircleCheck),
            palette.accent,
            gettext(locale, "The key works. Lyrics light up word by word.").to_string(),
        ),
        SpicyKeyStatus::FromEnvironment => (
            Some(theme::Icon::CircleCheck),
            palette.accent,
            gettext(
                locale,
                "The key from the SPICY_LYRICS_KEY environment variable works. It wins over a saved key.",
            )
            .to_string(),
        ),
        SpicyKeyStatus::Rejected => (
            Some(theme::Icon::CircleX),
            palette.danger,
            gettext(
                locale,
                "Spicy Lyrics refused the key. Check that you copied the whole secret key and that “No origin header” is on.",
            )
            .to_string(),
        ),
        SpicyKeyStatus::Unchecked(reason) => (
            Some(theme::Icon::CircleAlert),
            palette.warning,
            // Translators: {error} is the original failure detail.
            gettext(locale, "Couldn't check the key: {error}").replace("{error}", reason),
        ),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        match icon {
            Some(icon) => {
                theme::icon(ui, icon, 16.0, color);
            }
            None => {
                theme::spinner(ui, 16.0, color);
            }
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(text)
                    .font(theme::regular(13.5))
                    .color(color),
            )
            .wrap(),
        );
    });
}

/// Setup for the Spicy Lyrics key: what it is for, how to get one, the field
/// to paste it in, and where it goes.
fn spicy_lyrics(app: &mut App, ui: &mut egui::Ui) {
    use crate::model::SpicyKeyStatus;
    let palette = app.palette;
    let locale = app.locale;
    let status = app.spicy_key_status.clone();
    let Some(Dialog::SpicyLyrics { key, reveal }) = &mut app.dialog else {
        return;
    };
    ui.set_width(480.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        theme::icon(ui, theme::Icon::Sparkles, 22.0, palette.accent);
        theme::text(ui, "Spicy Lyrics", theme::bold(20.0), palette.text);
    });
    ui.add_space(6.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(gettext(
                locale,
                "Word-by-word karaoke lyrics, with romaji for Japanese songs, from the free Spicy Lyrics service. It needs your own key, which takes two minutes to make.",
            ))
            .font(theme::regular(14.0))
            .color(palette.secondary),
        )
        .wrap(),
    );
    ui.add_space(12.0);
    spicy_status(ui, &palette, locale, &status);
    ui.add_space(14.0);

    theme::text(
        ui,
        gettext(locale, "Get a free key"),
        theme::semibold(14.0),
        palette.text,
    );
    ui.add_space(6.0);
    let steps = [
        gettext(
            locale,
            "Open the Spicy Lyrics developer dashboard and sign in.",
        )
        .to_string(),
        gettext(locale, "Create an application.").to_string(),
        gettext(
            locale,
            "In the application's key settings, turn on “No origin header”. Desktop apps need it.",
        )
        .to_string(),
        gettext(locale, "Copy the secret key. It starts with sl_sk_.").to_string(),
        gettext(locale, "Paste it below and choose Save.").to_string(),
    ];
    for (number, step) in steps.iter().enumerate() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (badge, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(badge.center(), 10.0, palette.surface_active);
            ui.painter().text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                (number + 1).to_string(),
                theme::semibold(11.5),
                palette.text,
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(step)
                        .font(theme::regular(13.5))
                        .color(palette.text),
                )
                .wrap(),
            );
        });
        ui.add_space(4.0);
    }
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if theme::soft_button(
            ui,
            &palette,
            Some(theme::Icon::ExternalLink),
            &gettext(locale, "Open the dashboard"),
            false,
        )
        .clicked()
        {
            app.actions
                .push(Action::OpenUrl(SPICY_DASHBOARD_URL.to_owned()));
        }
        if theme::soft_button(
            ui,
            &palette,
            Some(theme::Icon::ExternalLink),
            &gettext(locale, "How keys work"),
            false,
        )
        .clicked()
        {
            app.actions.push(Action::OpenUrl(SPICY_KEYS_URL.to_owned()));
        }
    });
    ui.add_space(14.0);

    theme::text(
        ui,
        gettext(locale, "Secret key"),
        theme::medium(13.0),
        palette.secondary,
    );
    ui.add_space(2.0);
    let field = ui
        .horizontal(|ui| {
            let field = Frame::new()
                .fill(palette.surface)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width() - 70.0);
                    super::widgets::text_edit(
                        ui,
                        locale,
                        egui::TextEdit::singleline(key)
                            .id(egui::Id::new("spicy-lyrics-key"))
                            .password(!*reveal)
                            .hint_text(egui::RichText::new("sl_sk_…").color(palette.dim))
                            .font(theme::regular(14.0))
                            .frame(egui::Frame::NONE)
                            .desired_width(f32::INFINITY),
                    )
                })
                .inner;
            let label = if *reveal {
                gettext(locale, "Hide")
            } else {
                gettext(locale, "Show")
            };
            if theme::link(
                ui,
                label.to_string(),
                theme::medium(13.0),
                palette.secondary,
            )
            .clicked()
            {
                *reveal = !*reveal;
            }
            field
        })
        .inner;
    ui.add_space(8.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(gettext(
                locale,
                "The key stays on this computer, in Spotifast's config folder. Spotifast sends it only to api.spicylyrics.org, with the Spotify ID of the song you open the lyrics for. Spicy Lyrics is a separate service with its own terms.",
            ))
            .font(theme::regular(12.5))
            .color(palette.dim),
        )
        .wrap(),
    );
    if theme::link(
        ui,
        gettext(locale, "Spicy Lyrics terms").to_string(),
        theme::regular(12.5),
        palette.secondary,
    )
    .clicked()
    {
        app.actions
            .push(Action::OpenUrl(SPICY_TERMS_URL.to_owned()));
    }
    ui.add_space(18.0);

    let typed = key.trim().to_string();
    let submit = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let checking = status == SpicyKeyStatus::Checking;
    let stored = matches!(
        status,
        SpicyKeyStatus::Working | SpicyKeyStatus::Rejected | SpicyKeyStatus::Unchecked(_)
    );
    buttons(ui, |ui| {
        if checking {
            theme::spinner(ui, 18.0, palette.accent);
        } else if (theme::pill_button(ui, &palette, &gettext(locale, "Save"), true).clicked()
            || submit)
            && !typed.is_empty()
        {
            app.actions.push(Action::SaveSpicyKey(typed.clone()));
        }
        if theme::pill_button(ui, &palette, &gettext(locale, "Close"), false).clicked() {
            app.actions.push(Action::CloseDialog);
        }
        if stored
            && !checking
            && theme::pill_button(ui, &palette, &gettext(locale, "Remove key"), false).clicked()
        {
            app.actions.push(Action::RemoveSpicyKey);
        }
    });
}

fn create_playlist(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let locale = app.locale;
    let busy = app.playlist_busy;
    let Some(Dialog::CreatePlaylist {
        name,
        public,
        add_uris,
    }) = &mut app.dialog
    else {
        return;
    };
    theme::text(
        ui,
        gettext(locale, "New playlist"),
        theme::bold(20.0),
        palette.text,
    );
    ui.add_space(12.0);
    theme::text(
        ui,
        gettext(locale, "Name"),
        theme::medium(13.0),
        palette.secondary,
    );
    let field = text_field(
        ui,
        &palette,
        locale,
        "playlist-name",
        name,
        &gettext(locale, "My playlist"),
        true,
    );
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        super::widgets::switch(ui, &palette, &gettext(locale, "Public playlist"), public);
        theme::text(
            ui,
            gettext(locale, "Public playlist"),
            theme::regular(14.0),
            palette.text,
        );
    });
    if !add_uris.is_empty() {
        ui.add_space(6.0);
        let count = add_uris.len();
        theme::text(
            ui,
            ngettext(
                locale,
                // Translators: {count} is the number of songs.
                "{count} song will be added.",
                "{count} songs will be added.",
                count as u32,
            )
            .replace("{count}", &count.to_string()),
            theme::regular(13.0),
            palette.secondary,
        );
    }
    ui.add_space(20.0);
    let submit = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let name_value = name.trim().to_string();
    let public_value = *public;
    let uris = add_uris.clone();
    buttons(ui, |ui| {
        if busy {
            theme::spinner(ui, 18.0, palette.accent);
        } else {
            let create = theme::pill_button(ui, &palette, &gettext(locale, "Create"), true)
                .clicked()
                || submit;
            if create && !name_value.is_empty() {
                app.actions.push(Action::CreatePlaylist {
                    name: name_value.clone(),
                    public: public_value,
                    add_uris: uris.clone(),
                });
            }
            if theme::pill_button(ui, &palette, &gettext(locale, "Cancel"), false).clicked() {
                app.actions.push(Action::CloseDialog);
            }
        }
    });
}

fn edit_playlist(app: &mut App, ui: &mut egui::Ui) {
    let existing_image = if let Some(Dialog::EditPlaylist { id, .. }) = &app.dialog {
        app.library
            .playlists
            .get()
            .and_then(|playlists| playlists.iter().find(|playlist| &playlist.id == id))
            .and_then(|playlist| crate::api::models::pick_image(&playlist.images, 100))
            .map(str::to_owned)
    } else {
        None
    };
    let palette = app.palette;
    let locale = app.locale;
    let busy = app.playlist_busy;
    let Some(Dialog::EditPlaylist {
        cover,
        id,
        name,
        description,
        public,
    }) = &mut app.dialog
    else {
        return;
    };
    theme::text(
        ui,
        gettext(locale, "Edit details"),
        theme::bold(20.0),
        palette.text,
    );
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .max_height((ui.ctx().content_rect().height() - 210.0).max(100.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(selected) = &cover.selection {
                    ui.ctx()
                        .include_bytes(selected.uri.clone(), selected.jpeg.clone());
                    ui.add(
                        egui::Image::new(selected.uri.clone())
                            .texture_options(crate::images::COVER_TEXTURE)
                            .fit_to_exact_size(egui::vec2(72.0, 72.0))
                            .alt_text(gettext(locale, "Selected playlist cover")),
                    );
                } else if let Some(url) = &existing_image {
                    ui.add(
                        egui::Image::new(url)
                            .texture_options(crate::images::COVER_TEXTURE)
                            .fit_to_exact_size(egui::vec2(72.0, 72.0))
                            .alt_text(gettext(locale, "Current playlist cover")),
                    );
                }
                ui.vertical(|ui| {
                    if cover.request.is_some() || cover.uploading.is_some() {
                        theme::spinner(ui, 18.0, palette.accent);
                        theme::text(
                            ui,
                            if cover.uploading.is_some() {
                                gettext(locale, "Uploading cover…")
                            } else {
                                gettext(locale, "Choosing cover…")
                            },
                            theme::regular(13.0),
                            palette.secondary,
                        );
                    } else {
                        if theme::pill_button(ui, &palette, &gettext(locale, "Change cover"), false)
                            .clicked()
                        {
                            app.actions.push(Action::ChoosePlaylistCover(id.clone()));
                        }
                        if cover.selection.is_some()
                            && theme::pill_button(
                                ui,
                                &palette,
                                &gettext(locale, "Upload cover"),
                                true,
                            )
                            .clicked()
                        {
                            app.actions.push(Action::UploadPlaylistCover(id.clone()));
                        }
                    }
                });
            });
            if let Some(error) = &cover.error {
                ui.add(egui::Label::new(egui::RichText::new(error).color(palette.text)).wrap());
            }
            ui.add_space(10.0);
            theme::text(
                ui,
                gettext(locale, "Name"),
                theme::medium(13.0),
                palette.secondary,
            );
            text_field(
                ui,
                &palette,
                locale,
                "edit-name",
                name,
                &gettext(locale, "Playlist name"),
                true,
            );
            ui.add_space(10.0);
            theme::text(
                ui,
                gettext(locale, "Description"),
                theme::medium(13.0),
                palette.secondary,
            );
            Frame::new()
                .fill(palette.surface)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    super::widgets::text_edit(
                        ui,
                        locale,
                        egui::TextEdit::multiline(description)
                            .id(egui::Id::new("edit-description"))
                            .hint_text(
                                egui::RichText::new(gettext(locale, "Optional description"))
                                    .color(palette.dim),
                            )
                            .font(theme::regular(14.0))
                            .frame(egui::Frame::NONE)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // Unknown shows as off; only a change of the switch is sent, so
                // a playlist nothing has described keeps whatever it was.
                let mut shown = public.unwrap_or(false);
                if super::widgets::switch(
                    ui,
                    &palette,
                    &gettext(locale, "Public playlist"),
                    &mut shown,
                )
                .changed()
                {
                    *public = Some(shown);
                }
                theme::text(
                    ui,
                    gettext(locale, "Public playlist"),
                    theme::regular(14.0),
                    palette.text,
                );
            });
        });
    ui.add_space(20.0);
    let id = id.clone();
    let busy = busy || cover.uploading.is_some() || cover.request.is_some();
    let name_value = name.trim().to_string();
    let description_value = description.trim().to_string();
    let public_value = *public;
    buttons(ui, |ui| {
        if busy {
            theme::spinner(ui, 18.0, palette.accent);
        } else {
            if theme::pill_button(ui, &palette, &gettext(locale, "Save"), true).clicked()
                && !name_value.is_empty()
            {
                app.actions.push(Action::UpdatePlaylist {
                    id: id.clone(),
                    name: name_value.clone(),
                    description: description_value.clone(),
                    public: public_value,
                });
            }
            if theme::pill_button(ui, &palette, &gettext(locale, "Cancel"), false).clicked() {
                app.actions.push(Action::CloseDialog);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::duplicate_message;
    use crate::api::models::{PlayableItem, Track};
    use crate::i18n::Locale;

    fn song(uri: &str, name: &str) -> PlayableItem {
        PlayableItem::Track(Track {
            uri: uri.into(),
            name: name.into(),
            ..Default::default()
        })
    }

    #[test]
    fn duplicate_dialog_names_the_song() {
        let items = vec![song("spotify:track:honey", "Honey")];
        let message = duplicate_message(
            Locale::English,
            "The best music ever",
            &items,
            &["spotify:track:honey".into()],
        );

        assert_eq!(
            message,
            "“Honey” is already in “The best music ever”. Add it again?"
        );
    }

    #[test]
    fn duplicate_dialog_names_only_the_duplicates_in_a_selection() {
        let items = vec![
            song("spotify:track:honey", "Honey"),
            song("spotify:track:new", "New song"),
        ];
        let message = duplicate_message(
            Locale::English,
            "The best music ever",
            &items,
            &["spotify:track:honey".into()],
        );

        assert_eq!(
            message,
            "“Honey” is already in “The best music ever”. Add them anyway?"
        );
    }
}
