mod browse;
mod controls;
mod details;
mod dialogs;
mod icons;
mod style;
mod tools;

use crate::app::{App, Message, Page};
use crate::i18n::t;
use controls::button;
use iced::widget::{
    Column, column, container, image, mouse_area, opaque, row, space, stack, text, text_input,
    tooltip,
};
use iced::{Alignment, Element, Fill, Font, Length};
use icons::Icon;
use style::*;

pub fn app_theme(app: &App) -> iced::Theme {
    theme(app.theme_mode)
}

pub(super) fn small<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into()).size(12).style(text_color(MUTED))
}
pub(super) fn bold<'a>(value: impl Into<String>, size: u32) -> iced::widget::Text<'a> {
    text(value.into()).size(size).font(Font {
        weight: iced::font::Weight::Semibold,
        ..Font::with_name("System Font")
    })
}
pub(super) fn shortened(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(max).collect::<String>())
    }
}

fn header(app: &App) -> Element<'_, Message> {
    static LOGO: std::sync::LazyLock<image::Handle> = std::sync::LazyLock::new(|| {
        image::Handle::from_bytes(include_bytes!("../../assets/brew-store-logo.png").as_slice())
    });
    let navigation = row![
        button(icons::label(app, Icon::Search, t("Hľadať"), false))
            .padding([6, 13])
            .style(choice(app.page == Page::Search))
            .on_press(Message::Navigate(Page::Search)),
        button(icons::label(
            app,
            Icon::Installed,
            t("Nainštalované"),
            false
        ))
        .padding([6, 13])
        .style(choice(app.page == Page::Installed))
        .on_press(Message::Navigate(Page::Installed)),
    ]
    .spacing(2);
    let mut actions = row![].spacing(6).align_y(Alignment::Center);
    if app.page == Page::Installed {
        let enabled = app.can_upgrade_all();
        let description = if app.busy() {
            t("Prebieha operácia…").to_owned()
        } else if app.inventory_loading || !app.updates_fresh {
            t("Kontrolujem aktualizácie…").to_owned()
        } else if enabled {
            format!("{} · ⌘⇧U", t("Aktualizovať všetko"))
        } else {
            t("Všetko je aktuálne alebo pripnuté.").to_owned()
        };
        actions = actions.push(
            tooltip(
                button(icons::label_tint(
                    app,
                    Icon::UpdateAll,
                    t("Aktualizovať všetko"),
                    if enabled { BG } else { MUTED },
                ))
                .padding([7, 12])
                .style(action(false))
                .on_press_maybe(
                    enabled.then_some(Message::Propose(crate::brew::Action::UpgradeAll)),
                ),
                text(description).size(12),
                iced::widget::tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(450)),
        );
    }
    actions = actions
        .push(icons::button(
            app,
            Icon::Tools,
            format!("{} · ⌘,", t("Nástroje Apple")),
            (!app.busy() && !app.tools_repair.as_ref().is_some_and(|r| r.checking))
                .then_some(Message::CheckTools),
            false,
        ))
        .push(icons::button(
            app,
            Icon::Refresh,
            format!("{} · ⌘R", t("Obnoviť")),
            (!app.busy() && !app.inventory_loading && !app.catalog_loading)
                .then_some(Message::Refresh),
            false,
        ));
    let brand = row![
        image(LOGO.clone()).width(24).height(24),
        bold("Brew Store", 13),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::FillPortion(1));
    let body = row![
        brand,
        container(navigation).padding(3).style(panel(HOVER, false)),
        row![space().width(Fill), actions].width(Length::FillPortion(1)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    mouse_area(
        container(body)
            .padding([4, 16])
            .width(Fill)
            .height(44)
            .style(|theme| {
                let mut style = panel(SURFACE, false)(theme);
                style.border.radius = 0.into();
                style
            }),
    )
    .on_press(Message::DragWindow)
    .into()
}

fn notices(app: &App) -> Element<'_, Message> {
    let errors = if app.page == Page::Installed {
        vec![
            app.brew.as_ref().err(),
            app.inventory_error.as_ref(),
            app.updates_error.as_ref(),
        ]
    } else {
        vec![
            app.catalog_error.as_ref(),
            app.brew.as_ref().err(),
            app.inventory_error.as_ref(),
        ]
    };
    let mut content = Column::new().spacing(8);
    for error in errors.into_iter().flatten().take(2) {
        content = content.push(
            container(text(shortened(error, 220)).size(13).style(text_color(RED)))
                .padding(12)
                .width(Fill)
                .style(panel(iced::color!(0xf9edeb), false)),
        );
    }
    if let Some(notice) = &app.notice {
        content = content.push(
            container(
                row![
                    text(notice).size(13).width(Fill),
                    button(crate::i18n::t("Zavrieť"))
                        .style(choice(false))
                        .on_press(Message::DismissNotice)
                ]
                .spacing(12),
            )
            .padding(12)
            .width(Fill)
            .style(panel(HOVER, false)),
        );
    }
    content.into()
}

fn content(app: &App) -> Element<'_, Message> {
    if app.selected.is_some() {
        return details::view(app);
    }
    let placeholder = if app.page == Page::Search {
        t("Vyhľadať v Homebrew…")
    } else {
        t("Filtrovať nainštalované…")
    };
    let query = row![
        icons::glyph(app, Icon::Search, 18, MUTED),
        text_input(placeholder, app.query())
            .id("catalog-search")
            .on_input(Message::Search)
            .on_submit(Message::SubmitSearch)
            .padding([8, 4])
            .size(14)
            .style(input)
            .width(Fill),
        if app.query().is_empty() {
            container(small("⌘F")).padding([0, 6]).into()
        } else {
            icons::button(
                app,
                Icon::Close,
                t("Vymazať vyhľadávanie"),
                Some(Message::Search(String::new())),
                false,
            )
        },
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let top = column![
        container(query)
            .padding([2, 10])
            .style(panel(SURFACE, true)),
        container(browse::filters(app)).height(32),
    ]
    .spacing(12);
    container(
        column![top, browse::package_list(app)]
            .spacing(10)
            .max_width(1120),
    )
    .padding([20, 24])
    .center_x(Fill)
    .height(Fill)
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let mut body = column![header(app)].height(Fill).width(Fill);
    let has_notice = app.notice.is_some()
        || app.brew.is_err()
        || app.inventory_error.is_some()
        || (app.page == Page::Search && app.catalog_error.is_some())
        || (app.page == Page::Installed && app.updates_error.is_some());
    if has_notice {
        body = body.push(container(notices(app)).padding([8, 30]));
    }
    body = body.push(content(app));
    if app.operation.is_some() {
        body = body.push(dialogs::operation(app));
    }
    let mut base: Element<'_, Message> = container(body)
        .width(Fill)
        .height(Fill)
        .style(panel(BG, false))
        .into();
    if app.tools_repair.is_some() {
        base = stack![
            base,
            opaque(
                container(container(tools::view(app)).width(680))
                    .center_x(Fill)
                    .center_y(Fill)
                    .padding(24)
                    .style(|_| iced::widget::container::Style {
                        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.24).into()),
                        ..Default::default()
                    })
            )
        ]
        .into();
    }
    if let Some(action) = &app.confirmation {
        stack![base, dialogs::confirmation(app, action)].into()
    } else {
        base
    }
}
