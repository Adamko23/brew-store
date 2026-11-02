use super::controls::button;
use super::icons::{self, Icon};
use super::{bold, shortened, small, style::*};
use crate::app::{App, Confirmation, Effect, Message};
use crate::brew::{Action, progress_snapshot};
use iced::widget::{column, container, opaque, row, scrollable, space, text};
use iced::{Alignment, Element, Fill, Font};

pub fn operation(app: &App) -> Element<'_, Message> {
    let Some(operation) = &app.operation else {
        return space().height(0).into();
    };
    let data = progress_snapshot(&operation.progress);
    let (status, color) = match &operation.result {
        None => (
            format!(
                "{} · {} s",
                operation.title,
                operation.started.elapsed().as_secs()
            ),
            ACCENT,
        ),
        Some(Ok(_)) => (
            crate::i18n::message("Hotovo · {}", &[(operation.title).to_string()], &[]),
            GREEN,
        ),
        Some(Err(_)) if operation.effect == Effect::Applied => (
            if operation.action.is_removal() {
                crate::i18n::t("Balíček je odstránený. Následné kroky Homebrew zlyhali; podrobnosti sú vo výpise.")
                    .into()
            } else if matches!(operation.action, Action::UpgradeAll) {
                crate::i18n::t("Homebrew už neeviduje ďalšie aktualizácie. Následné kroky skončili chybou; podrobnosti sú vo výpise.").into()
            } else {
                crate::i18n::message(
                    "{} je vo verzii {}. Následné kroky Homebrew zlyhali.",
                    &[
                        (operation
                            .action
                            .key()
                            .map(|key| key.name.as_str())
                            .unwrap_or("Homebrew"))
                        .to_string(),
                        (operation.target_version.as_deref().unwrap_or("—")).to_string(),
                    ],
                    &[],
                )
            },
            AMBER,
        ),
        Some(Err(error)) => (shortened(error, 180), RED),
    };
    let mut bar = row![
        icons::glyph(
            app,
            if operation
                .result
                .as_ref()
                .is_some_and(|result| result.is_ok())
            {
                Icon::Check
            } else if operation.result.is_some() {
                Icon::Warning
            } else {
                Icon::Update
            },
            16,
            color
        ),
        text(status).size(13).style(text_color(color)).width(Fill),
        icons::button(
            app,
            Icon::Log,
            if app.show_log {
                crate::i18n::t("Skryť výpis")
            } else {
                crate::i18n::t("Podrobnosti")
            },
            Some(Message::ToggleLog),
            false
        ),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if app.retry_action().is_some() {
        bar = bar.push(icons::button(
            app,
            Icon::Refresh,
            crate::i18n::t("Opakovať pokus"),
            Some(Message::RetryOperation),
            false,
        ));
    }
    if data.developer_tools.is_some() && matches!(operation.result, Some(Err(_))) && !app.busy() {
        bar = bar.push(icons::button(
            app,
            Icon::Tools,
            crate::i18n::t("Vyriešiť nástroje Apple"),
            (!app
                .tools_repair
                .as_ref()
                .is_some_and(|repair| repair.checking))
            .then_some(Message::CheckTools),
            false,
        ));
    }
    if !app.busy() {
        bar = bar.push(icons::button(
            app,
            Icon::Close,
            crate::i18n::t("Zavrieť"),
            Some(Message::DismissOperation),
            false,
        ));
    }
    let mut content = column![bar].spacing(12);
    if app.show_log {
        content = content.push(
            scrollable(
                text(data.lines.iter().cloned().collect::<Vec<_>>().join("\n"))
                    .font(Font::MONOSPACE)
                    .size(12),
            )
            .anchor_bottom()
            .height(120),
        );
    }
    container(content)
        .padding([14, 30])
        .width(Fill)
        .style(panel(HOVER, false))
        .into()
}

pub fn confirmation<'a>(app: &App, confirmation: &'a Confirmation) -> Element<'a, Message> {
    let ready = !app.busy()
        && (!matches!(confirmation, Confirmation::Package(_))
            || (app.inventory_fresh && !app.inventory_loading));
    let (title, description, accept, removal) = match confirmation {
        Confirmation::Package(command) => {
            let removal = command.is_removal();
            let description = if matches!(command, Action::UpgradeAll) {
                crate::i18n::t(
                    "Homebrew jedným príkazom aktualizuje všetky dostupné balíčky a ich závislosti. Rešpektuje pripnuté verzie a svoje pravidlá pre aplikácie s vlastným aktualizátorom. Priebeh môže chvíľu trvať.",
                )
            } else if matches!(command, Action::RemoveStale(_)) {
                crate::i18n::t(
                    "Súbor aplikácie už chýba. Homebrew odstráni zvyšky svojho inštalačného záznamu cez odinštaláciu s --force. Stav sa pred vykonaním znovu overí.",
                )
            } else if removal {
                crate::i18n::t(
                    "Homebrew odinštaluje vybraný balíček a skontroluje jeho závislosti.",
                )
            } else {
                crate::i18n::t(
                    "Homebrew vykoná túto operáciu aj s potrebnými zmenami závislostí. Ak bude treba oprávnenie správcu, zobrazí sa systémový dialóg.",
                )
            };
            let accept = if matches!(command, Action::RemoveStale(_)) {
                crate::i18n::t("Odstrániť záznam")
            } else if removal {
                crate::i18n::t("Odinštalovať")
            } else {
                crate::i18n::t("Potvrdiť")
            };
            (command.label(), description, accept, removal)
        }
        Confirmation::DeveloperTools(plan) => (
            plan.title(),
            match plan {
                crate::tools::Plan::Update(_) => crate::i18n::t(
                    "Nainštaluje sa iba vybraný balíček nástrojov od Apple. macOS môže vyžiadať oprávnenie správcu. Po dokončení sa overí nainštalovaná verzia.",
                ),
                crate::tools::Plan::Installer => crate::i18n::t(
                    "Otvorí sa systémový inštalátor Apple. Dokonči jeho kroky a prípadné licenčné podmienky, potom skontroluj stav znovu.",
                ),
            },
            crate::i18n::t("Pokračovať"),
            false,
        ),
    };
    let mut content = column![
        bold(title, 23),
        small(description),
        space().height(6),
        row![
            space().width(Fill),
            button(crate::i18n::t("Zrušiť"))
                .id("confirmation-cancel")
                .padding([11, 17])
                .style(choice(false))
                .on_press(Message::Cancel),
            button(accept)
                .id("confirmation-accept")
                .padding([11, 17])
                .style(action(removal))
                .on_press_maybe(ready.then_some(Message::Confirm))
        ]
        .spacing(10),
    ]
    .spacing(20);
    if !ready {
        content = content.push(small(crate::i18n::t(
            "Pred vykonaním overujem aktuálny stav Homebrew…",
        )));
    }
    opaque(
        container(
            container(content)
                .padding(30)
                .width(510)
                .style(panel(SURFACE, true)),
        )
        .center_x(Fill)
        .center_y(Fill)
        .style(|_| container::Style {
            background: Some(iced::Color::from_rgba(0.12, 0.17, 0.14, 0.25).into()),
            ..Default::default()
        }),
    )
}
