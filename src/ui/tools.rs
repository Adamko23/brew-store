use super::controls::button;
use super::{bold, small, style::*};
use crate::app::{App, Message};
use crate::brew::progress_snapshot;
use iced::widget::{column, container, row, space, text};
use iced::{Alignment, Element, Fill, Font};

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(repair) = &app.tools_repair else {
        return space().into();
    };
    let mut header = row![
        bold(crate::i18n::t("Command Line Tools od Apple"), 16),
        space().width(Fill)
    ]
    .align_y(Alignment::Center)
    .spacing(12);
    if !repair.running {
        header = header.push(
            button(crate::i18n::t("Zavrieť"))
                .id("tools-close")
                .style(choice(false))
                .on_press(Message::DismissTools),
        );
    }
    let mut body = column![header].spacing(10);
    if repair.checking {
        body = body.push(small(crate::i18n::t(
            "Kontrolujem nainštalované nástroje a ponuku Apple. Môže to trvať do minúty.",
        )));
    }
    if let Some(report) = &repair.report {
        let version = report
            .installed_version
            .as_deref()
            .unwrap_or(if report.files_present {
                crate::i18n::t("Súbory sú na disku, verzia sa nedá overiť")
            } else {
                crate::i18n::t("Nástroje nie sú evidované")
            });
        body = body.push(small(crate::i18n::message(
            "Nainštalované: {version}",
            &[],
            &[("version", version.to_string())],
        )));
        if repair.result.is_none() && !repair.running {
            body = body.push(small(&report.note));
        }
        let label = if let Some(update) = &report.update {
            crate::i18n::message(
                "Nainštalovať nástroje {}",
                &[(update.version).to_string()],
                &[],
            )
        } else {
            crate::i18n::t("Systémový inštalátor").into()
        };
        body = body.push(
            row![
                button(text(label).size(13))
                    .id("tools-primary")
                    .padding([8, 12])
                    .style(action(false))
                    .on_press_maybe((!app.busy()).then_some(Message::ProposeTools)),
                button(crate::i18n::t("Skontrolovať znovu"))
                    .id("tools-recheck")
                    .padding([8, 12])
                    .style(choice(false))
                    .on_press_maybe((!app.busy()).then_some(Message::CheckTools)),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
        body = body.push(
            row![
                button(crate::i18n::t("Aktualizácia softvéru ↗"))
                    .id("tools-settings")
                    .padding([8, 12])
                    .style(choice(false))
                    .on_press(Message::ToolsSettings),
                button(crate::i18n::t("Stiahnuť od Apple ↗"))
                    .id("tools-download")
                    .padding([8, 12])
                    .style(choice(false))
                    .on_press(Message::OpenHomepage(crate::tools::DOWNLOADS.into())),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    if repair.running {
        body = body.push(small(crate::i18n::t(
            "Apple inštaluje nástroje. Počas tejto operácie ponechaj aplikáciu otvorenú.",
        )));
        let data = progress_snapshot(&repair.progress);
        let last_lines = data.lines.iter().rev().take(3).cloned().collect::<Vec<_>>();
        if !last_lines.is_empty() {
            body = body.push(
                text(last_lines.into_iter().rev().collect::<Vec<_>>().join("\n"))
                    .size(12)
                    .font(Font::MONOSPACE),
            );
        }
    }
    if let Some(result) = &repair.result {
        let (message, color) = match result {
            Ok(message) => (message, ACCENT),
            Err(error) => (error, RED),
        };
        body = body.push(text(message).size(13).style(text_color(color)));
        if result.is_err() {
            let data = progress_snapshot(&repair.progress);
            if let Some(line) = data.lines.back() {
                body = body.push(
                    text(super::shortened(line, 220))
                        .size(12)
                        .font(Font::MONOSPACE),
                );
            }
        }
    }
    container(body)
        .padding(24)
        .width(Fill)
        .style(panel(SURFACE, true))
        .into()
}
