use super::controls::button;
use super::icons::{self, Icon};
use super::{bold, shortened, small, style::*};
use crate::app::{App, Message, PAGE_SIZE, Page, Scope};
use crate::assets::format_size;
use crate::brew::Action;
use crate::i18n::t;
use crate::model::{Kind, Package};
use iced::widget::{Column, column, container, image, row, scrollable, space, text};
use iced::{Alignment, ContentFit, Element, Fill};

pub fn filters(app: &App) -> Element<'_, Message> {
    let mut options = row![].spacing(6).align_y(Alignment::Center);
    for scope in Scope::ALL
        .into_iter()
        .filter(|_| app.page == Page::Installed || !app.query().is_empty())
    {
        options = options.push(
            button(icons::label(
                app,
                match scope {
                    Scope::Apps => Icon::Apps,
                    Scope::Tools => Icon::Terminal,
                    Scope::All => Icon::All,
                },
                scope.label(),
                false,
            ))
            .padding([4, 10])
            .style(choice(app.scope == scope))
            .on_press(Message::Scope(scope)),
        );
    }
    let count = if app.search_loading {
        crate::i18n::t("Vyhľadávam…").into()
    } else if app.page == Page::Search && app.query().trim().is_empty() {
        String::new()
    } else if app.page == Page::Installed && !app.inventory_fresh && app.inventory.is_empty() {
        crate::i18n::t("Načítavam inventár…").into()
    } else {
        let count = app.matches.len();
        format!(
            "{count} {}",
            match count {
                1 => crate::i18n::t("položka"),
                2..=4 => crate::i18n::t("položky"),
                _ => crate::i18n::t("položiek"),
            }
        )
    };
    options.push(space().width(Fill)).push(small(count)).into()
}

pub fn avatar<'a>(app: &App, package: &Package, size: u32) -> Element<'a, Message> {
    if let Some(path) = app.visuals.get(&package.key).and_then(|v| v.icon.as_ref()) {
        return image(image::Handle::from_path(path))
            .width(size)
            .height(size)
            .content_fit(ContentFit::Contain)
            .into();
    }
    container(icons::glyph(
        app,
        if package.key.kind == Kind::Formula {
            Icon::Terminal
        } else {
            Icon::Apps
        },
        size * 3 / 5,
        MUTED,
    ))
    .center_x(size)
    .center_y(size)
    .style(panel(HOVER, false))
    .into()
}

pub(super) fn action_buttons<'a>(app: &App, package: &Package) -> Element<'a, Message> {
    let visual = app.visuals.get(&package.key);
    let missing = visual.is_some_and(|visual| visual.app_missing);
    let presence_known =
        package.key.kind == Kind::Formula || package.app_paths.is_empty() || visual.is_some();
    let ready = app.can_change() && presence_known;
    let required_macos = app.required_macos(package);
    let mut buttons = row![].spacing(4).align_y(Alignment::Center);
    if package.installed.is_some()
        && !missing
        && visual.is_some_and(|visual| visual.app_bundle.is_some())
    {
        buttons = buttons.push(icons::button(
            app,
            Icon::Open,
            format!("{} {} · ⌘O", t("Otvoriť"), package.title),
            (!app.busy()).then_some(Message::OpenApplication(package.key.clone())),
            false,
        ));
    }
    let upgrade_label = || {
        required_macos.map(|version| {
            crate::i18n::message(
                "Vyžaduje macOS {version}",
                &[],
                &[("version", version.into())],
            )
        })
    };
    if package.installed.is_none() {
        buttons = buttons.push(icons::button(
            app,
            if required_macos.is_some() {
                Icon::Warning
            } else {
                Icon::Update
            },
            upgrade_label().unwrap_or_else(|| format!("{} {}", t("Nainštalovať"), package.title)),
            (app.can_change() && !package.disabled && required_macos.is_none())
                .then_some(Message::Propose(Action::Install(package.key.clone()))),
            false,
        ));
    } else if missing {
        buttons = buttons.push(icons::button(
            app,
            Icon::Trash,
            t("Odstrániť záznam"),
            ready.then_some(Message::Propose(Action::RemoveStale(package.key.clone()))),
            true,
        ));
    } else {
        if package.outdated && !package.is_pinned() {
            buttons = buttons.push(icons::button(
                app,
                if required_macos.is_some() {
                    Icon::Warning
                } else {
                    Icon::Update
                },
                upgrade_label()
                    .unwrap_or_else(|| format!("{} {}", t("Aktualizovať"), package.title)),
                (ready && app.updates_fresh && !package.disabled && required_macos.is_none())
                    .then_some(Message::Propose(Action::Upgrade(package.key.clone()))),
                false,
            ));
        }
        buttons = buttons.push(icons::button(
            app,
            Icon::Trash,
            format!("{} {}", t("Odinštalovať"), package.title),
            ready.then_some(Message::Propose(Action::Uninstall(package.key.clone()))),
            false,
        ));
    }
    buttons.into()
}

fn package_row<'a>(app: &'a App, package: &'a Package) -> Element<'a, Message> {
    let version = if package.installed.is_some() {
        if package.outdated {
            format!(
                "{} → {}",
                shortened(&package.installed_version(), 20),
                shortened(&package.version, 20)
            )
        } else {
            package.installed_version()
        }
    } else {
        package.version.clone()
    };
    let missing = app.visuals.get(&package.key).is_some_and(|v| v.app_missing);
    let subtitle = if missing {
        crate::i18n::t("Záznam v Homebrew · súbor aplikácie chýba").into()
    } else if app.page == Page::Installed {
        format!(
            "{} · {}{}",
            package.key.kind.label(),
            version,
            if package.is_pinned() {
                crate::i18n::t(" · pripnuté")
            } else {
                ""
            }
        )
    } else {
        format!(
            "{} · {}",
            package.key.name,
            shortened(&package.description, 90)
        )
    };
    let consumers = app.graph.direct_users(&package.key).len()
        + app.graph.runtime_users(&package.key).len()
        + app.graph.indirect_users(&package.key).len();
    let subtitle = if app.page == Page::Installed && !missing && consumers > 0 {
        let template = match consumers {
            1 => "používa ho {} balíček",
            2..=4 => "používajú ho {} balíčky",
            _ => "používa ho {} balíčkov",
        };
        format!(
            "{subtitle} · {}",
            crate::i18n::message(template, &[consumers.to_string()], &[])
        )
    } else {
        subtitle
    };
    let subtitle = if let Some(version) = app.required_macos(package) {
        format!(
            "{subtitle} · {}",
            crate::i18n::message(
                "Vyžaduje macOS {version}",
                &[],
                &[("version", version.into())]
            )
        )
    } else {
        subtitle
    };
    let label = row![
        avatar(app, package, 40),
        column![
            bold(shortened(&package.title, 52), 15),
            text(subtitle).size(12).style(text_color(MUTED))
        ]
        .spacing(4)
        .width(Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let mut row = row![
        button(label)
            .padding([8, 8])
            .width(Fill)
            .style(choice(app.selected.as_ref() == Some(&package.key)))
            .on_press(Message::Select(package.key.clone()))
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if app.page == Page::Installed {
        let size = app
            .visuals
            .get(&package.key)
            .map(|v| v.bytes.map(format_size).unwrap_or_else(|| "—".into()))
            .unwrap_or_else(|| crate::i18n::t("Meriam…").into());
        row = row.push(
            small(size)
                .width(82)
                .align_x(iced::alignment::Horizontal::Right),
        );
    }
    row = row.push(
        container(action_buttons(app, package))
            .width(112)
            .align_x(Alignment::End),
    );
    column![
        row,
        container(space().height(1))
            .width(Fill)
            .style(panel(LINE, false))
    ]
    .spacing(5)
    .into()
}

pub fn package_list(app: &App) -> Element<'_, Message> {
    let start = app.offset;
    let end = (start + PAGE_SIZE).min(app.matches.len());
    let content: Element<'_, Message> = if app.matches.is_empty() {
        let (title, subtitle) = if app.page == Page::Search && app.query().trim().is_empty() {
            (
                crate::i18n::t("Čo si chceš nainštalovať?"),
                crate::i18n::t("Začni písať názov alebo popis. Výsledky sa zobrazia tu."),
            )
        } else if app.search_loading {
            (crate::i18n::t("Vyhľadávam…"), "")
        } else if app.inventory_loading && app.page == Page::Installed {
            (
                crate::i18n::t("Načítavam nainštalované balíčky"),
                crate::i18n::t("Údaje čítam z Homebrew na tomto Macu."),
            )
        } else if app.catalog_loading && app.page == Page::Search {
            (
                crate::i18n::t("Načítavam katalóg"),
                crate::i18n::t("Výsledky sa zobrazia po jeho načítaní."),
            )
        } else {
            (
                crate::i18n::t("Žiadne výsledky"),
                crate::i18n::t("Skús iný názov, popis alebo typ balíčka."),
            )
        };
        container(
            column![
                icons::glyph(app, Icon::Search, 48, MUTED),
                bold(title, 20),
                small(subtitle)
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .center_x(Fill)
        .center_y(Fill)
        .into()
    } else {
        scrollable(
            Column::with_children(
                app.matches[start..end]
                    .iter()
                    .map(|&i| package_row(app, &app.packages[i])),
            )
            .spacing(5)
            .padding(iced::Padding {
                right: 14.0,
                ..iced::Padding::ZERO
            }),
        )
        .height(Fill)
        .into()
    };
    let count = if app.matches.is_empty() {
        String::new()
    } else {
        crate::i18n::message(
            "{}–{} z {}",
            &[
                (start + 1).to_string(),
                (end).to_string(),
                (app.matches.len()).to_string(),
            ],
            &[],
        )
    };
    let mut paging = row![small(count), space().width(Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
    if app.matches.len() > PAGE_SIZE {
        paging = paging
            .push(icons::button(
                app,
                Icon::Back,
                t("Späť"),
                (start > 0).then_some(Message::Paginate(start.saturating_sub(PAGE_SIZE))),
                false,
            ))
            .push(icons::button(
                app,
                Icon::Forward,
                t("Ďalej"),
                (end < app.matches.len()).then_some(Message::Paginate(end)),
                false,
            ));
    }
    column![content, paging]
        .spacing(16)
        .height(Fill)
        .width(Fill)
        .into()
}
