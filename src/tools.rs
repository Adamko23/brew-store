use crate::brew::{SharedProgress, append_log, prepare_askpass, run_logged};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub const DOWNLOADS: &str = "https://developer.apple.com/download/all/?q=Command%20Line%20Tools";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Issue {
    pub recommended: Option<String>,
}

impl Issue {
    pub fn from_line(line: &str) -> Option<Self> {
        let lower = line.to_ascii_lowercase();
        if !lower.contains("command line tools")
            || ![
                "outdated",
                "newer",
                "too old",
                "not installed",
                "must be installed",
                "should download",
                "install the command line tools",
                "no sdk",
                "not support",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
        {
            return None;
        }
        let recommended = line
            .split_once("Command Line Tools for Xcode ")
            .map(|(_, value)| value.trim_end_matches('.').trim())
            .filter(|value| numeric_version(value).is_some())
            .map(str::to_owned);
        Some(Self { recommended })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    label: String,
    pub version: String,
}

impl Update {
    fn from_label(label: &str) -> Option<Self> {
        let suffix = label.strip_prefix("Command Line Tools for Xcode")?.trim();
        let version = suffix.rsplit('-').next()?.trim();
        numeric_version(version)?;
        if !suffix
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == ' ')
        {
            return None;
        }
        Some(Self {
            label: label.to_owned(),
            version: version.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Update(Update),
    Installer,
}

impl Plan {
    pub fn title(&self) -> String {
        match self {
            Self::Update(update) => crate::i18n::message(
                "Nainštalovať Command Line Tools {}",
                &[(update.version).to_string()],
                &[],
            ),
            Self::Installer => crate::i18n::t("Otvoriť inštalátor Command Line Tools").into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub installed_version: Option<String>,
    pub files_present: bool,
    pub update: Option<Update>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub message: String,
    pub installed_version: Option<String>,
}

impl Report {
    pub fn plan(&self) -> Plan {
        self.update
            .clone()
            .map(Plan::Update)
            .unwrap_or(Plan::Installer)
    }
}

#[derive(Clone)]
struct Commands {
    softwareupdate: PathBuf,
    pkgutil: PathBuf,
    sudo: PathBuf,
    xcode_select: PathBuf,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            softwareupdate: "/usr/sbin/softwareupdate".into(),
            pkgutil: "/usr/sbin/pkgutil".into(),
            sudo: "/usr/bin/sudo".into(),
            xcode_select: "/usr/bin/xcode-select".into(),
        }
    }
}

fn numeric_version(value: &str) -> Option<Vec<u32>> {
    if value.is_empty() || value.len() > 64 {
        return None;
    }
    value
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                None
            } else {
                part.parse().ok()
            }
        })
        .collect()
}

pub(crate) fn at_least(candidate: &str, minimum: &str) -> bool {
    let (Some(left), Some(right)) = (numeric_version(candidate), numeric_version(minimum)) else {
        return false;
    };
    (0..left.len().max(right.len()))
        .map(|i| left.get(i).copied().unwrap_or(0))
        .cmp((0..left.len().max(right.len())).map(|i| right.get(i).copied().unwrap_or(0)))
        != std::cmp::Ordering::Less
}

pub async fn system_version() -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("/usr/bin/sw_vers")
            .arg("--productVersion")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    numeric_version(&version).map(|_| version)
}

fn offered_macos(output: &str) -> Option<String> {
    let mut macos = false;
    for line in output.lines() {
        if let Some(label) = line.trim().strip_prefix("* Label: ") {
            macos = label.starts_with("macOS ");
        }
        if macos && let Some((_, value)) = line.split_once("Version: ") {
            let version = value.split(',').next()?.trim();
            return numeric_version(version).map(|_| version.to_owned());
        }
    }
    None
}

fn select_update(output: &str, required: Option<&str>, installed: Option<&str>) -> Option<Update> {
    output
        .lines()
        .filter_map(|line| Update::from_label(line.trim().strip_prefix("* Label: ")?))
        .filter(|update| {
            required.is_none_or(|minimum| at_least(&update.version, minimum))
                && installed.is_none_or(|version| !at_least(version, &update.version))
        })
        .max_by(|a, b| {
            if at_least(&a.version, &b.version) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        })
}

async fn installed_version(commands: &Commands) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&commands.pkgutil)
            .arg("--pkg-info=com.apple.pkg.CLTools_Executables")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let value = line.strip_prefix("version: ")?.trim();
            numeric_version(value).map(|_| value.to_owned())
        })
}

pub async fn check(issue: Issue) -> Report {
    let commands = Commands::default();
    let installed_version = installed_version(&commands).await;
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        Command::new(&commands.softwareupdate)
            .arg("--list")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await;
    let (update, note) = match output {
        Ok(Ok(output)) if output.status.success() => {
            let update = select_update(&String::from_utf8_lossy(&output.stdout),
                issue.recommended.as_deref(), installed_version.as_deref());
            let note = if update.is_some() { crate::i18n::t("Apple ponúka kompatibilný balíček. Aplikácia ho môže nainštalovať samostatne.") }
            else { crate::i18n::t("Apple teraz neponúka vhodnú samostatnú aktualizáciu Command Line Tools. Skús systémový inštalátor alebo stiahnutie od Apple.") };
            let mut note = note.to_owned();
            if update.is_none() && let Some(version) = offered_macos(&String::from_utf8_lossy(&output.stdout)) {
                note.push_str(&crate::i18n::message(" V ponuke je aj macOS {version}; ide o samostatnú aktualizáciu systému. Zoznam Apple neuvádza, prečo chýba balíček nástrojov.", &[], &[("version", version.to_string())]));
            }
            (update, note)
        }
        Err(_) => (None, crate::i18n::t("Kontrola Apple neodpovedala do minúty. Môžeš ju zopakovať alebo otvoriť systémový inštalátor.").into()),
        _ => (None, crate::i18n::t("Dostupnosť balíčka sa nepodarilo overiť. Použi systémový inštalátor alebo aktualizácie macOS.").into()),
    };
    Report {
        installed_version,
        files_present: Path::new("/Library/Developer/CommandLineTools/usr/bin/clang").is_file(),
        update,
        note,
    }
}

pub async fn run(
    plan: Plan,
    directory: PathBuf,
    progress: SharedProgress,
) -> Result<Completion, String> {
    run_with(plan, directory, progress, Commands::default()).await
}

async fn run_with(
    plan: Plan,
    directory: PathBuf,
    progress: SharedProgress,
    commands: Commands,
) -> Result<Completion, String> {
    match plan {
        Plan::Installer => {
            append_log(&progress, "$ xcode-select --install".into());
            let status = run_logged(
                Command::new(&commands.xcode_select).arg("--install"),
                progress,
            )
            .await?;
            if status.success() {
                Ok(Completion { message: crate::i18n::t("Systémový inštalátor je otvorený. Dokonči jeho kroky a potom zvoľ Skontrolovať znovu; inštalácia ešte nie je potvrdená.").into(), installed_version: None })
            } else {
                Err(crate::i18n::t("Inštalátor sa neotvoril. Ak macOS nástroje už eviduje, použi Aktualizáciu softvéru alebo stiahnutie od Apple. Podrobnosti sú vo výpise.").into())
            }
        }
        Plan::Update(update) => {
            if Update::from_label(&update.label).as_ref() != Some(&update) {
                return Err(crate::i18n::t("Neplatný balíček Command Line Tools.").into());
            }
            let helper = prepare_askpass(&directory)
                .await?
                .ok_or(crate::i18n::t("Chýba systémové overenie správcu."))?;
            append_log(
                &progress,
                format!("$ softwareupdate --install {}", update.label),
            );
            let status = run_logged(
                Command::new(&commands.sudo)
                    .args(["--askpass"])
                    .arg(&commands.softwareupdate)
                    .arg("--install")
                    .arg(&update.label)
                    .env("SUDO_ASKPASS", helper),
                progress,
            )
            .await?;
            if !status.success() {
                return Err(crate::i18n::t("Apple nedokončil inštaláciu nástrojov. Podrobnosti sú vo výpise; ďalší postup je dostupný v Aktualizácii softvéru.").into());
            }
            match installed_version(&commands).await {
                Some(version) if at_least(&version, &update.version) => Ok(Completion { message: crate::i18n::message("Command Line Tools {version} sú nainštalované a ich záznam je overený.", &[], &[("version", version.to_string())]), installed_version: Some(version) }),
                _ => Err(crate::i18n::t("Proces Apple skončil, ale požadovanú verziu nástrojov sa nepodarilo potvrdiť. Skontroluj stav znovu.").into()),
            }
        }
    }
}

pub async fn open_settings() -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.Software-Update-Settings.extension")
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(crate::i18n::t("Aktualizáciu softvéru sa nepodarilo otvoriť.").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn only_compatible_stable_clt_updates_can_be_selected() {
        let list = "* Label: macOS 27.2-26B123\n* Label: Command Line Tools for Xcode-26.4\n* Label: Command Line Tools for Xcode-27.0\n* Label: Command Line Tools for Xcode-27.2 beta\n";
        assert_eq!(
            select_update(list, Some("27.0"), None).unwrap().version,
            "27.0"
        );
        assert!(select_update(list, Some("27.1"), None).is_none());
        assert!(select_update(list, None, Some("27.0.0.0.1")).is_none());
        assert!(Update::from_label("Command Line Tools for Xcode-27.0;id").is_none());
        assert!(select_update("* Label: macOS 27.2-26B123", None, None).is_none());
        assert_eq!(
            offered_macos(
                "* Label: macOS 27.2-26B123\n  Title: macOS 27.2, Version: 27.2, Size: 123KiB, Action: restart,"
            ),
            Some("27.2".into())
        );
    }

    #[test]
    fn recognizes_homebrew_clt_diagnostics_without_treating_all_xcode_logs_as_errors() {
        assert_eq!(
            Issue::from_line("You should download the Command Line Tools for Xcode 27.0.")
                .unwrap()
                .recommended
                .as_deref(),
            Some("27.0")
        );
        assert!(Issue::from_line("Error: Your Command Line Tools are too outdated.").is_some());
        assert!(Issue::from_line("Installing Command Line Tools for Xcode 27.0").is_none());
        assert!(Issue::from_line("Xcode: 27.2").is_none());
    }

    fn fake(directory: &Path, name: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[tokio::test]
    async fn apple_update_uses_only_the_selected_label_and_requires_receipt_verification() {
        let directory = tempfile::tempdir().unwrap();
        let sudo = fake(directory.path(), "sudo", "printf '%s\\n' \"$@\"; exit 0");
        let pkgutil = fake(directory.path(), "pkgutil", "echo 'version: 26.4.0'");
        let commands = Commands {
            sudo,
            pkgutil: pkgutil.clone(),
            ..Commands::default()
        };
        let update = Update::from_label("Command Line Tools for Xcode-27.0").unwrap();
        let progress = SharedProgress::default();
        assert!(
            run_with(
                Plan::Update(update.clone()),
                directory.path().into(),
                progress.clone(),
                commands.clone()
            )
            .await
            .is_err()
        );
        let lines = crate::brew::progress_snapshot(&progress).lines;
        assert!(lines.iter().any(|line| line == "--install"));
        assert!(lines.iter().any(|line| line == &update.label));
        assert!(
            !lines
                .iter()
                .any(|line| ["--all", "--restart", "--agree-to-license"].contains(&line.as_str()))
        );
        fake(directory.path(), "pkgutil", "echo 'version: 27.0.0.0.1'");
        assert!(
            run_with(
                Plan::Update(update),
                directory.path().into(),
                progress,
                commands
            )
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn opening_the_native_installer_never_claims_installation_is_complete() {
        let directory = tempfile::tempdir().unwrap();
        let xcode_select = fake(
            directory.path(),
            "xcode-select",
            "[ \"$1\" = --install ] || exit 99; echo requested",
        );
        let result = run_with(
            Plan::Installer,
            directory.path().into(),
            SharedProgress::default(),
            Commands {
                xcode_select,
                ..Commands::default()
            },
        )
        .await
        .unwrap();
        assert!(result.message.contains("ešte nie je potvrdená"));
        assert!(result.installed_version.is_none());
    }
}
