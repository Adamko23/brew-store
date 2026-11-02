use super::controls;
use super::style::*;
use crate::app::{App, Message};
use iced::widget::{container, image, row, text, tooltip};
use iced::{Alignment, Color, Element};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Search,
    Installed,
    Refresh,
    Tools,
    Open,
    Update,
    UpdateAll,
    Trash,
    Close,
    Back,
    Forward,
    External,
    Log,
    Apps,
    Terminal,
    All,
    Warning,
    Check,
}

impl Icon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Search => include_bytes!("../../assets/ui-icons/search.png"),
            Self::Installed => include_bytes!("../../assets/ui-icons/installed.png"),
            Self::Refresh => include_bytes!("../../assets/ui-icons/refresh.png"),
            Self::Tools => include_bytes!("../../assets/ui-icons/tools.png"),
            Self::Open => include_bytes!("../../assets/ui-icons/open.png"),
            Self::Update => include_bytes!("../../assets/ui-icons/update.png"),
            Self::UpdateAll => include_bytes!("../../assets/ui-icons/update-all.png"),
            Self::Trash => include_bytes!("../../assets/ui-icons/trash.png"),
            Self::Close => include_bytes!("../../assets/ui-icons/close.png"),
            Self::Back => include_bytes!("../../assets/ui-icons/back.png"),
            Self::Forward => include_bytes!("../../assets/ui-icons/forward.png"),
            Self::External => include_bytes!("../../assets/ui-icons/external.png"),
            Self::Log => include_bytes!("../../assets/ui-icons/log.png"),
            Self::Apps => include_bytes!("../../assets/ui-icons/apps.png"),
            Self::Terminal => include_bytes!("../../assets/ui-icons/terminal.png"),
            Self::All => include_bytes!("../../assets/ui-icons/all.png"),
            Self::Warning => include_bytes!("../../assets/ui-icons/warning.png"),
            Self::Check => include_bytes!("../../assets/ui-icons/check.png"),
        }
    }
}

type IconCache = HashMap<(Icon, [u8; 4]), image::Handle>;
static CACHE: LazyLock<Mutex<IconCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn glyph<'a>(app: &App, icon: Icon, size: u32, color: Color) -> Element<'a, Message> {
    let color = adapt(&theme(app.theme_mode), color);
    let rgba = [color.r, color.g, color.b, color.a].map(|value| (value * 255.0).round() as u8);
    let handle = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry((icon, rgba))
        .or_insert_with(|| {
            let mut mask = ::image::load_from_memory(icon.bytes())
                .expect("bundled SF Symbol")
                .into_rgba8();
            for pixel in mask.pixels_mut() {
                let alpha = ((pixel[3] as u16 * rgba[3] as u16) / 255) as u8;
                pixel.0 = [rgba[0], rgba[1], rgba[2], alpha];
            }
            image::Handle::from_rgba(mask.width(), mask.height(), mask.into_raw())
        })
        .clone();
    image(handle).width(size).height(size).into()
}

pub fn label<'a>(
    app: &App,
    icon: Icon,
    value: impl Into<String>,
    primary: bool,
) -> Element<'a, Message> {
    label_tint(app, icon, value, if primary { BG } else { TEXT })
}

pub fn label_tint<'a>(
    app: &App,
    icon: Icon,
    value: impl Into<String>,
    color: Color,
) -> Element<'a, Message> {
    row![glyph(app, icon, 16, color), text(value.into()).size(13)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

pub fn button<'a>(
    app: &App,
    icon: Icon,
    description: impl Into<String>,
    message: Option<Message>,
    danger: bool,
) -> Element<'a, Message> {
    let color = if message.is_none() {
        MUTED
    } else if danger {
        RED
    } else {
        TEXT
    };
    tooltip(
        controls::button(
            container(glyph(app, icon, 18, color))
                .center_x(20)
                .center_y(20),
        )
        .padding(5)
        .style(choice(false))
        .on_press_maybe(message),
        container(text(description.into()).size(12))
            .padding([5, 8])
            .style(panel(SURFACE, true)),
        tooltip::Position::Bottom,
    )
    .gap(5)
    .delay(std::time::Duration::from_millis(450))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_symbols_have_transparent_padding_and_visible_strokes() {
        for icon in [
            Icon::Search,
            Icon::Open,
            Icon::Update,
            Icon::Trash,
            Icon::UpdateAll,
        ] {
            let image = ::image::load_from_memory(icon.bytes())
                .unwrap()
                .into_rgba8();
            assert!(image.pixels().any(|pixel| pixel[3] > 128));
            assert!(image.pixels().any(|pixel| pixel[3] == 0));
        }
    }
}
