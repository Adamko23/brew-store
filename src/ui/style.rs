use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Theme, color};

pub const BG: Color = color!(0xf5f5f7);
pub const SURFACE: Color = Color::WHITE;
pub const HOVER: Color = color!(0xebebef);
pub const LINE: Color = color!(0xdedee4);
pub const TEXT: Color = color!(0x1d1d1f);
pub const MUTED: Color = color!(0x64646b);
pub const ACCENT: Color = color!(0x286451);
pub const ACCENT_BG: Color = color!(0xe8f2ec);
pub const GREEN: Color = color!(0x246746);
pub const RED: Color = color!(0xa73932);
pub const AMBER: Color = color!(0x8c5f21);

pub fn adapt(theme: &Theme, color: Color) -> Color {
    if theme.palette().background == BG {
        return color;
    }
    if color == BG {
        color!(0x1c1c1e)
    } else if color == SURFACE {
        color!(0x29292c)
    } else if color == HOVER {
        color!(0x343438)
    } else if color == LINE {
        color!(0x414146)
    } else if color == TEXT {
        color!(0xf5f5f7)
    } else if color == MUTED {
        color!(0xaaaab2)
    } else if color == ACCENT || color == GREEN {
        color!(0x96d5b5)
    } else if color == ACCENT_BG {
        color!(0x263d34)
    } else if color == RED {
        color!(0xffafa5)
    } else if color == AMBER {
        color!(0xe8be7b)
    } else if color == color!(0xf9edeb) {
        color!(0x422c2b)
    } else {
        color
    }
}

pub fn theme(mode: iced::theme::Mode) -> Theme {
    use std::sync::LazyLock;
    static LIGHT: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "Brew Store Light",
            iced::theme::Palette {
                background: BG,
                text: TEXT,
                primary: ACCENT,
                success: GREEN,
                warning: color!(0x8c5f21),
                danger: RED,
            },
        )
    });
    static DARK: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "Brew Store Dark",
            iced::theme::Palette {
                background: color!(0x1c1c1e),
                text: color!(0xf5f5f7),
                primary: color!(0x96d5b5),
                success: color!(0x96d5b5),
                warning: color!(0xe8be7b),
                danger: color!(0xffafa5),
            },
        )
    });
    if mode == iced::theme::Mode::Dark {
        DARK.clone()
    } else {
        LIGHT.clone()
    }
}

pub fn text_color(color: Color) -> impl Fn(&Theme) -> iced::widget::text::Style {
    move |theme| iced::widget::text::Style {
        color: Some(adapt(theme, color)),
    }
}

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.background = iced::Color::TRANSPARENT.into();
    style.border.width = 0.0;
    style.placeholder = adapt(theme, MUTED);
    style.value = adapt(theme, TEXT);
    style.border.radius = 6.0.into();
    style
}

pub fn panel(background: Color, border: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: Some(adapt(theme, background).into()),
        text_color: Some(adapt(theme, TEXT)),
        border: Border {
            color: adapt(theme, LINE),
            width: if border { 1.0 } else { 0.0 },
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub fn choice(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| button::Style {
        background: Some(
            adapt(
                theme,
                if selected {
                    SURFACE
                } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    HOVER
                } else {
                    Color::TRANSPARENT
                },
            )
            .into(),
        ),
        text_color: adapt(
            theme,
            if matches!(status, button::Status::Disabled) {
                MUTED
            } else {
                TEXT
            },
        ),
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn action(danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let disabled = matches!(status, button::Status::Disabled);
        let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(adapt(
                theme,
                if disabled {
                    HOVER
                } else if danger {
                    color!(0xf9edeb)
                } else if hover {
                    if theme.palette().background == BG {
                        color!(0x1c503e)
                    } else {
                        color!(0xb1e6ca)
                    }
                } else {
                    ACCENT
                },
            ))),
            text_color: if disabled {
                adapt(theme, MUTED)
            } else if danger {
                adapt(theme, RED)
            } else {
                adapt(theme, BG)
            },
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
