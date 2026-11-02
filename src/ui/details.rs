use super::controls::button;
use super::icons::{self, Icon};
use super::{bold, browse, small, style::*};
use crate::app::{App, Message, Page};
use crate::assets::format_size;
use crate::model::{Key, Package};
use iced::widget::{Column, column, container, row, scrollable, space, text};
use iced::{Alignment, Element, Fill};

fn property<'a>(label: &'static str, value: impl Into<String>) -> Element<'a, Message> {
    column![small(label), text(value.into()).size(15)]
        .spacing(7)
        .into()
}

fn relation<'a>(app: &App, key: &Key) -> Element<'a, Message> {
    let title = app
        .packages
        .iter()
        .find(|p| &p.key == key)
        .map(|p| p.title.clone())
        .unwrap_or_else(|| key.name.clone());
    button(icons::label(app, Icon::Forward, title, false))
        .padding([7, 0])
        .width(Fill)
        .style(choice(false))
        .on_press(Message::Select(key.clone()))
        .into()
}

fn relationships<'a>(app: &App, package: &Package) -> Element<'a, Message> {
    let mut needs = column![bold(crate::i18n::t("Potrebuje"), 16)]
        .spacing(8)
        .width(Fill);
    let dependencies = if package.installed.is_some() {
        app.graph.needs(&package.key)
    } else {
        &package.dependencies
    };
    if !dependencies.is_empty() {
        needs = needs.push(small(crate::i18n::t("Priamo deklarované:")));
    }
    for key in dependencies {
        needs = needs.push(relation(app, key));
    }
    let runtime_needs = app.graph.runtime_needs(&package.key);
    if !runtime_needs.is_empty() {
        needs = needs.push(small(crate::i18n::t(
            "Ďalšie runtime závislosti zo záznamu inštalácie:",
        )));
        for key in runtime_needs {
            needs = needs.push(relation(app, key));
        }
    }
    let unresolved = app.graph.unresolved(&package.key);
    for key in unresolved {
        needs = needs.push(small(crate::i18n::message(
            "{} · nenájdené v inventári",
            &[(key.name).to_string()],
            &[],
        )));
    }
    if dependencies.is_empty() && runtime_needs.is_empty() && unresolved.is_empty() {
        needs = needs.push(small(crate::i18n::t(
            "Homebrew neuvádza žiadny ďalší potrebný balíček.",
        )));
    }
    let mut users = column![bold(crate::i18n::t("Používa ho"), 16)]
        .spacing(8)
        .width(Fill);
    let direct = app.graph.direct_users(&package.key);
    let runtime = app.graph.runtime_users(&package.key);
    let indirect = app.graph.indirect_users(&package.key);
    if !direct.is_empty() {
        users = users.push(small(crate::i18n::t("Priamo deklarované:")));
    }
    for key in direct {
        users = users.push(relation(app, key));
    }
    if !runtime.is_empty() {
        users = users.push(small(crate::i18n::t(
            "Ďalšie runtime väzby zo záznamov inštalácií:",
        )));
        for key in runtime {
            users = users.push(relation(app, key));
        }
        users = users.push(small(crate::i18n::t(
            "Zahŕňajú aj nepriame závislosti a väzby bez priamej deklarácie.",
        )));
    }
    if !indirect.is_empty() {
        users = users.push(small(crate::i18n::t("Nepriamo cez ďalšie závislosti:")));
        for key in indirect {
            users = users.push(relation(app, key));
        }
    }
    if direct.is_empty() && runtime.is_empty() && indirect.is_empty() {
        users = users.push(small(crate::i18n::t(
            "Žiadny nainštalovaný balíček Homebrew ho neuvádza ako závislosť.",
        )));
    }
    users = users.push(small(crate::i18n::t(
        "Používanie softvérom mimo Homebrew sa tu nedá určiť.",
    )));
    row![needs, users].spacing(40).into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(package) = app.selected_package() else {
        return space().into();
    };
    let back = if app.page == Page::Installed {
        crate::i18n::t("← Nainštalované")
    } else {
        crate::i18n::t("← Výsledky vyhľadávania")
    };
    let visual = app.visuals.get(&package.key);
    let hero = row![
        browse::avatar(app, package, 64),
        column![bold(&package.title, 24), small(&package.key.name)]
            .spacing(9)
            .width(Fill),
        browse::action_buttons(app, package),
    ]
    .spacing(22)
    .align_y(Alignment::Center);
    let mut content = column![
        button(icons::label(
            app,
            Icon::Back,
            back.trim_start_matches("← "),
            false
        ))
        .padding([8, 0])
        .style(choice(false))
        .on_press(Message::CloseDetail),
        space().height(8),
        hero,
        container(text(&package.description).size(16)).max_width(720),
    ]
    .spacing(22);
    if app.detail_loading {
        content = content.push(small(crate::i18n::t("Načítavam podrobnosti z Homebrew…")));
    }
    if let Some(error) = &app.detail_error {
        content = content.push(text(error).size(13).style(text_color(RED)));
    }
    if app.detail_from_installation {
        content = content.push(
            text(crate::i18n::t("Aktuálnu definíciu balíčka sa nepodarilo načítať. Zobrazené sú overené údaje nainštalovanej verzie; odinštalovanie zostáva dostupné."))
                .size(13)
                .style(text_color(AMBER)),
        );
    }
    if visual.is_some_and(|v| v.app_missing) {
        content = content.push(container(column![bold(crate::i18n::t("Súbor aplikácie na disku chýba"), 16),
            text(crate::i18n::t("Homebrew ešte eviduje inštalačný záznam. Akciu Odstrániť záznam použi na jeho vyčistenie cez Homebrew.")).size(14)].spacing(8))
            .padding(18).width(Fill).style(panel(HOVER, false)));
    }
    let installed = package
        .installed
        .as_ref()
        .map(|i| i.versions.join(", "))
        .unwrap_or_else(|| crate::i18n::t("Nie je nainštalovaná").into());
    let size = visual
        .and_then(|v| v.bytes)
        .map(format_size)
        .unwrap_or_else(|| crate::i18n::t("Nedostupná").into());
    let left = column![
        property(crate::i18n::t("Nainštalovaná verzia"), installed),
        property(crate::i18n::t("Veľkosť inštalácie na disku"), size)
    ]
    .spacing(24)
    .width(Fill);
    let right = column![
        property(
            crate::i18n::t(if app.detail_from_installation {
                "Verzia zo záznamu inštalácie"
            } else {
                "Verzia v Homebrew"
            }),
            &package.version,
        ),
        property(
            crate::i18n::t("Zdroj"),
            if package.tap.is_empty() {
                crate::i18n::t("Homebrew — zdroj neuvedený")
            } else {
                &package.tap
            }
        )
    ]
    .spacing(24)
    .width(Fill);
    content = content
        .push(
            container(space().height(1))
                .width(Fill)
                .style(panel(LINE, false)),
        )
        .push(row![left, right].spacing(40));
    if let Some(minimum) = &package.minimum_macos {
        content = content.push(property(crate::i18n::t("Minimálna verzia macOS"), minimum));
        if app.required_macos(package).is_some() {
            content = content.push(
                text(crate::i18n::message(
                    "Táto verzia vyžaduje macOS {minimum} alebo novší. Na tomto Macu je macOS {}.",
                    &[(app.macos_version.as_deref().unwrap_or("—")).to_string()],
                    &[("minimum", minimum.to_string())],
                ))
                .size(14)
                .style(text_color(AMBER)),
            );
        }
    }
    if package.disabled {
        content = content.push(small(crate::i18n::t(
            "Homebrew už tento balíček neponúka na novú inštaláciu.",
        )));
    }
    if package.is_pinned() {
        content = content.push(small(crate::i18n::t(
            "Verzia je pripnutá a neaktualizuje sa.",
        )));
    }
    if package.auto_updates {
        content = content.push(small(crate::i18n::t(
            "Aplikácia používa aj vlastný aktualizátor.",
        )));
    }
    content = content
        .push(
            container(space().height(1))
                .width(Fill)
                .style(panel(LINE, false)),
        )
        .push(relationships(app, package));
    if !package.caveats.is_empty() {
        content = content.push(property(
            crate::i18n::t("Poznámky Homebrew"),
            &package.caveats,
        ));
    }
    if !package.homepage.is_empty() {
        content = content.push(
            button(icons::label(
                app,
                Icon::External,
                crate::i18n::t("Web aplikácie ↗")
                    .trim_end_matches('↗')
                    .trim(),
                false,
            ))
            .padding([9, 0])
            .style(choice(false))
            .on_press(Message::OpenHomepage(package.homepage.clone())),
        );
    }
    let body: Column<'_, Message> = content;
    scrollable(
        container(container(body).max_width(980).width(Fill))
            .center_x(Fill)
            .padding([20, 28]),
    )
    .width(Fill)
    .height(Fill)
    .into()
}
