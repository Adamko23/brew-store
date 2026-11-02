use crate::model::{Key, Outdated, Package, parse_inventory, parse_outdated};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

const MAX_LOG_LINES: usize = 400;
const MAX_LINE_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct Brew {
    pub executable: PathBuf,
    gate: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub packages: Vec<Package>,
    pub outdated: Result<Vec<Outdated>, String>,
    pub cellar: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Details {
    pub package: Package,
    pub from_installation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Install(Key),
    Upgrade(Key),
    UpgradeAll,
    Uninstall(Key),
    RemoveStale(Key),
}

impl Action {
    pub fn key(&self) -> Option<&Key> {
        Some(match self {
            Self::Install(key)
            | Self::Upgrade(key)
            | Self::Uninstall(key)
            | Self::RemoveStale(key) => key,
            Self::UpgradeAll => return None,
        })
    }

    pub fn args(&self) -> Result<Vec<String>, String> {
        let (verb, key) = match self {
            Self::Install(key) => ("install", key),
            Self::Upgrade(key) => ("upgrade", key),
            Self::UpgradeAll => return Ok(vec!["upgrade".into()]),
            Self::Uninstall(key) => ("uninstall", key),
            Self::RemoveStale(key) => {
                key.validate()?;
                if key.kind != crate::model::Kind::Cask {
                    return Err(crate::i18n::t(
                        "Odstránenie neúplného záznamu je určené iba pre cask aplikácie.",
                    )
                    .into());
                }
                return Ok(vec![
                    "uninstall".into(),
                    "--cask".into(),
                    "--force".into(),
                    key.name.clone(),
                ]);
            }
        };
        key.validate()?;
        Ok(vec![verb.into(), key.kind.flag().into(), key.name.clone()])
    }

    pub fn label(&self) -> String {
        match self {
            Self::Install(key) => {
                crate::i18n::message("Nainštalovať {}", &[(key.name).to_string()], &[])
            }
            Self::Upgrade(key) => {
                crate::i18n::message("Aktualizovať {}", &[(key.name).to_string()], &[])
            }
            Self::UpgradeAll => crate::i18n::t("Aktualizovať všetko").into(),
            Self::Uninstall(key) => {
                crate::i18n::message("Odinštalovať {}", &[(key.name).to_string()], &[])
            }
            Self::RemoveStale(key) => {
                crate::i18n::message("Odstrániť záznam {}", &[(key.name).to_string()], &[])
            }
        }
    }

    pub fn is_removal(&self) -> bool {
        matches!(self, Self::Uninstall(_) | Self::RemoveStale(_))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub lines: VecDeque<String>,
    pub completed: usize,
    pub total: usize,
    pub current: String,
    pub developer_tools: Option<crate::tools::Issue>,
}

pub type SharedProgress = Arc<Mutex<Progress>>;

pub fn progress_snapshot(progress: &SharedProgress) -> Progress {
    progress.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub(crate) fn append_log(progress: &SharedProgress, line: String) {
    let line = clean_line(&line);
    if line.is_empty() {
        return;
    }
    let mut data = progress.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(issue) = crate::tools::Issue::from_line(&line)
        && (issue.recommended.is_some() || data.developer_tools.is_none())
    {
        data.developer_tools = Some(issue);
    }
    if data.lines.len() == MAX_LOG_LINES {
        data.lines.pop_front();
    }
    data.lines.push_back(line);
}

fn clean_line(line: &str) -> String {
    let mut output = String::new();
    let mut escape = false;
    for ch in line.chars() {
        if ch == '\u{1b}' {
            escape = true;
            continue;
        }
        if escape {
            if ch.is_ascii_alphabetic() || ch == '~' {
                escape = false;
            }
            continue;
        }
        if !ch.is_control() || ch == '\t' {
            output.push(ch);
        }
    }
    output.trim().to_owned()
}

async fn stream_log(
    mut stream: impl AsyncRead + Unpin,
    progress: SharedProgress,
) -> std::io::Result<()> {
    let mut chunk = [0; 1024];
    let mut line = Vec::new();
    loop {
        let len = stream.read(&mut chunk).await?;
        if len == 0 {
            break;
        }
        for &byte in &chunk[..len] {
            if byte == b'\n' || byte == b'\r' {
                append_log(&progress, String::from_utf8_lossy(&line).into_owned());
                line.clear();
            } else if line.len() < MAX_LINE_BYTES {
                line.push(byte);
            }
        }
    }
    if !line.is_empty() {
        append_log(&progress, String::from_utf8_lossy(&line).into_owned());
    }
    Ok(())
}

pub(crate) async fn run_logged(
    command: &mut Command,
    progress: SharedProgress,
) -> Result<std::process::ExitStatus, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            crate::i18n::message("Proces sa nedá spustiť: {e}", &[], &[("e", e.to_string())])
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or(crate::i18n::t("Chýba výstup procesu."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(crate::i18n::t("Chýba chybový výstup procesu."))?;
    let (out, err, status) = tokio::join!(
        stream_log(stdout, progress.clone()),
        stream_log(stderr, progress),
        child.wait(),
    );
    out.map_err(|e| e.to_string())?;
    err.map_err(|e| e.to_string())?;
    status.map_err(|e| e.to_string())
}

impl Brew {
    pub fn discover() -> Result<Self, String> {
        let mut candidates = vec![
            PathBuf::from("/opt/homebrew/bin/brew"),
            PathBuf::from("/usr/local/bin/brew"),
        ];
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|p| p.join("brew")));
        }
        let executable = candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or(crate::i18n::t(
                "Homebrew sa nenašiel. Nainštaluj ho z brew.sh a znovu otvor aplikáciu.",
            ))?;
        Ok(Self {
            executable,
            gate: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .env("HOMEBREW_NO_AUTO_UPDATE", "1")
            .env("HOMEBREW_NO_ANALYTICS", "1")
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .env("HOMEBREW_NO_COLOR", "1")
            .env_remove("HOMEBREW_UPGRADE_GREEDY")
            .env_remove("HOMEBREW_UPGRADE_GREEDY_CASKS")
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .kill_on_drop(true);
        command
    }

    async fn read(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let output =
            tokio::time::timeout(Duration::from_secs(180), self.command().args(args).output())
                .await
                .map_err(|_| {
                    crate::i18n::t("Homebrew neodpovedal do 180 sekúnd. Skús údaje obnoviť.")
                        .to_owned()
                })?
                .map_err(|e| {
                    crate::i18n::message(
                        "Homebrew sa nedá spustiť: {e}",
                        &[],
                        &[("e", e.to_string())],
                    )
                })?;
        if !output.status.success() {
            return Err(format!(
                "brew {}: {}",
                args.join(" "),
                clean_line(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        Ok(output.stdout)
    }

    async fn installed_packages(&self) -> Result<Vec<Package>, String> {
        parse_inventory(&self.read(&["info", "--json=v2", "--installed"]).await?)
    }

    pub async fn snapshot(&self) -> Result<Snapshot, String> {
        let _guard = self.gate.lock().await;
        let packages = self.installed_packages().await?;
        let outdated = match self.read(&["outdated", "--json=v2"]).await {
            Ok(bytes) => parse_outdated(&bytes),
            Err(error) => Err(error),
        };
        let cellar = self.read(&["--cellar"]).await?;
        let cellar = PathBuf::from(String::from_utf8_lossy(&cellar).trim());
        if !cellar.is_absolute() {
            return Err(crate::i18n::t("Homebrew nevrátil platný adresár Cellar.").into());
        }
        Ok(Snapshot {
            packages,
            outdated,
            cellar,
        })
    }

    pub async fn details(&self, key: &Key) -> Result<Details, String> {
        key.validate()?;
        let _guard = self.gate.lock().await;
        let current = self
            .read(&["info", "--json=v2", key.kind.flag(), &key.name])
            .await
            .and_then(|bytes| {
                parse_inventory(&bytes)?
                    .into_iter()
                    .find(|p| &p.key == key)
                    .ok_or_else(|| {
                        crate::i18n::t("Homebrew nevrátil detail vybraného balíčka.").into()
                    })
            });
        match current {
            Ok(package) => Ok(Details {
                package,
                from_installation: false,
            }),
            Err(original) => {
                // A formula can disappear or migrate to a cask while its old
                // keg remains installed. Never substitute the new package kind
                // or another tap. Homebrew's installed receipt owns this detail.
                let installed = self
                    .installed_packages()
                    .await
                    .map_err(|error| format!("{original}\n{error}"))?;
                installed
                    .into_iter()
                    .find(|p| &p.key == key && p.installed.is_some())
                    .map(|package| Details {
                        package,
                        from_installation: true,
                    })
                    .ok_or(original)
            }
        }
    }

    async fn formula_removal_args(&self, key: &Key) -> Result<Vec<String>, String> {
        let installed = self.installed_packages().await?;
        let token = key.name.rsplit('/').next().unwrap_or_default();
        let mut matches = installed.iter().filter(|package| {
            package.key.kind == crate::model::Kind::Formula
                && package.installed.is_some()
                && package.token == token
        });
        let package = matches
            .next()
            .filter(|package| &package.key == key)
            .filter(|_| matches.next().is_none())
            .ok_or(crate::i18n::t(
                "Stav Homebrew sa zmenil. Obnov údaje a vyber operáciu znovu.",
            ))?;
        // Formula racks have a flat token namespace. Homebrew resolves a fully
        // qualified name through today's formula definition before looking for
        // its keg, which fails after a formula-to-cask migration. Only use the
        // token after checking a fresh, unambiguous installed identity above.
        let token_key = Key {
            kind: crate::model::Kind::Formula,
            name: package.token.clone(),
        };
        token_key.validate()?;
        Ok(vec!["uninstall".into(), "--formula".into(), token_key.name])
    }

    pub async fn run(
        &self,
        actions: Vec<Action>,
        directory: PathBuf,
        progress: SharedProgress,
    ) -> Result<usize, String> {
        let _guard = self.gate.try_lock().map_err(|_| {
            crate::i18n::t("Homebrew už vykonáva inú operáciu. Počkaj na jej dokončenie.")
                .to_owned()
        })?;
        if actions.is_empty() {
            return Err(crate::i18n::t("Nie je vybraná žiadna operácia.").into());
        }
        // Validate the entire batch before launching its first process.
        let arguments: Vec<_> = actions.iter().map(Action::args).collect::<Result<_, _>>()?;
        let askpass = prepare_askpass(&directory).await?;
        {
            let mut data = progress.lock().unwrap_or_else(|e| e.into_inner());
            data.total = actions.len();
        }
        for (index, (action, mut args)) in actions.iter().zip(arguments).enumerate() {
            if let Action::Uninstall(key) = action
                && key.kind == crate::model::Kind::Formula
            {
                args = self.formula_removal_args(key).await?;
            }
            if let Action::RemoveStale(key) = action {
                let bytes = self
                    .read(&["info", "--json=v2", "--cask", &key.name])
                    .await?;
                let package = parse_inventory(&bytes)?
                    .into_iter()
                    .find(|p| &p.key == key)
                    .ok_or(crate::i18n::t(
                        "Homebrew nevrátil overiteľný záznam aplikácie.",
                    ))?;
                if package.installed.is_none() {
                    append_log(
                        &progress,
                        crate::i18n::t("Homebrew už inštalačný záznam neeviduje.").into(),
                    );
                    progress.lock().unwrap_or_else(|e| e.into_inner()).completed = index + 1;
                    continue;
                }
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default();
                let paths = crate::assets::app_candidates(&package.app_paths, &home);
                if paths.is_empty()
                    || paths
                        .iter()
                        .any(|path| !matches!(path.try_exists(), Ok(false)))
                {
                    return Err(crate::i18n::t("Aplikácia je na disku alebo jej chýbanie nemožno overiť. Obnov údaje a použi bežnú odinštaláciu.").into());
                }
            }
            progress.lock().unwrap_or_else(|e| e.into_inner()).current = action.label();
            append_log(&progress, format!("$ brew {}", args.join(" ")));
            let mut command = self.command();
            if matches!(action, Action::UpgradeAll) {
                command.env_remove("HOMEBREW_NO_AUTO_UPDATE");
            }
            if action.is_removal() {
                // A failed resolution must not trigger Homebrew's automatic
                // removal of unrelated orphaned dependencies either.
                command.env("HOMEBREW_NO_AUTOREMOVE", "1");
            }
            command
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(helper) = &askpass {
                command.env("SUDO_ASKPASS", helper);
            }
            let status = run_logged(&mut command, progress.clone()).await?;
            if !status.success() {
                let error = crate::i18n::message(
                    "{} zlyhalo ({}). Dokončené: {index}/{}. Podrobnosti sú vo výpise.",
                    &[
                        (action.label()).to_string(),
                        (status).to_string(),
                        (actions.len()).to_string(),
                    ],
                    &[("index", index.to_string())],
                );
                append_log(&progress, error.clone());
                return Err(error);
            }
            if action.is_removal() {
                let key = action.key().expect("a removal always has a package key");
                if self
                    .installed_packages()
                    .await?
                    .iter()
                    .any(|p| &p.key == key && p.installed.is_some())
                {
                    return Err(crate::i18n::t(
                        "Homebrew záznam po odstránení stále eviduje. Podrobnosti sú vo výpise.",
                    )
                    .into());
                }
            }
            progress.lock().unwrap_or_else(|e| e.into_inner()).completed = index + 1;
            append_log(&progress, crate::i18n::t("Operácia dokončená.").into());
        }
        Ok(actions.len())
    }
}

pub(crate) async fn prepare_askpass(directory: &Path) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|e| e.to_string())?;
        let path = directory.join("brew-store-askpass");
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o700);
        let mut file = options.open(&path).await.map_err(|e| {
            crate::i18n::message(
                "Nedá sa pripraviť systémový dialóg: {e}",
                &[],
                &[("e", e.to_string())],
            )
        })?;
        use tokio::io::AsyncWriteExt;
        let script: &[u8] = match crate::i18n::language() {
            crate::i18n::Language::English => include_bytes!("askpass-en.applescript"),
            crate::i18n::Language::Slovak => include_bytes!("askpass.applescript"),
        };
        file.write_all(script).await.map_err(|e| e.to_string())?;
        file.sync_all().await.map_err(|e| e.to_string())?;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some(path))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = directory;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;
    use std::os::unix::fs::PermissionsExt;

    fn fake_brew(script: &str) -> (tempfile::TempDir, Brew) {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("brew");
        std::fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        (
            directory,
            Brew {
                executable,
                gate: Arc::new(tokio::sync::Mutex::new(())),
            },
        )
    }

    fn sample() -> Key {
        Key {
            kind: Kind::Cask,
            name: "team/tap/sample".into(),
        }
    }

    fn legacy_formula() -> Key {
        Key {
            kind: Kind::Formula,
            name: "evcc-io/tap/evcc".into(),
        }
    }

    fn migrated_formula_fixture(keep_record: bool) -> (tempfile::TempDir, Brew) {
        let (directory, brew) = fake_brew("");
        // The tap now defines a cask, but the Mac still has its old formula.
        // Both kinds may share a token; the cask must never be selected instead.
        let inventory = serde_json::json!({
            "formulae": [{"name":"evcc", "full_name":"evcc-io/tap/evcc",
                "tap":"evcc-io/tap", "versions":{"stable":"0.132.1"},
                "installed":[{"version":"0.132.1"}]}],
            "casks": [{"token":"evcc", "full_token":"evcc-io/tap/evcc",
                "version":"0.317.2", "installed":"0.317.2"}]
        });
        std::fs::write(
            directory.path().join("inventory.json"),
            serde_json::to_vec(&inventory).unwrap(),
        )
        .unwrap();
        std::fs::write(
            &brew.executable,
            r#"#!/bin/sh
base=${0%/*}
if [ "$1" = info ]; then
    if [ "$#" -eq 3 ] && [ "$2" = --json=v2 ] && [ "$3" = --installed ]; then
        if [ -f "$base/removed" ]; then
            printf '{"formulae":[],"casks":[{"token":"evcc","full_token":"evcc-io/tap/evcc","installed":"0.317.2","version":"0.317.2"}]}'
        else
            cat "$base/inventory.json"
        fi
    else
        echo 'Error: No available formula with the name "evcc-io/tap/evcc".' >&2
        exit 1
    fi
elif [ "$1" = uninstall ]; then
    : > "$base/mutation-attempted"
    [ "$#" -eq 3 ] && [ "$2" = --formula ] && [ "$3" = evcc ] || exit 99
    [ "$HOMEBREW_NO_AUTOREMOVE" = 1 ] || exit 98
    MARK_REMOVED
else
    exit 97
fi
"#
            .replace(
                "MARK_REMOVED",
                if keep_record {
                    ":"
                } else {
                    ": > \"$base/removed\""
                },
            ),
        )
        .unwrap();
        (directory, brew)
    }

    #[tokio::test]
    async fn formula_uninstall_uses_verified_installed_token_when_the_definition_is_gone() {
        let (directory, brew) = migrated_formula_fixture(false);
        let progress = SharedProgress::default();
        assert_eq!(
            brew.run(
                vec![Action::Uninstall(legacy_formula())],
                directory.path().into(),
                progress.clone(),
            )
            .await
            .unwrap(),
            1
        );
        assert!(directory.path().join("removed").exists());
        assert!(
            progress_snapshot(&progress)
                .lines
                .iter()
                .any(|line| line == "$ brew uninstall --formula evcc")
        );
    }

    #[tokio::test]
    async fn formula_uninstall_refuses_changed_ambiguous_or_cask_only_inventory() {
        for formulae in [
            serde_json::json!([{"name":"evcc","full_name":"other/tap/evcc","installed":[{"version":"1"}]}]),
            serde_json::json!([
                {"name":"evcc","full_name":"evcc-io/tap/evcc","installed":[{"version":"1"}]},
                {"name":"evcc","full_name":"other/tap/evcc","installed":[{"version":"1"}]}
            ]),
            serde_json::json!([]),
        ] {
            let (directory, brew) = migrated_formula_fixture(false);
            let inventory = serde_json::json!({"formulae":formulae,
                "casks":[{"token":"evcc","full_token":"evcc-io/tap/evcc","installed":"2","version":"2"}]});
            std::fs::write(
                directory.path().join("inventory.json"),
                serde_json::to_vec(&inventory).unwrap(),
            )
            .unwrap();
            assert!(
                brew.run(
                    vec![Action::Uninstall(legacy_formula())],
                    directory.path().into(),
                    SharedProgress::default(),
                )
                .await
                .is_err()
            );
            assert!(!directory.path().join("mutation-attempted").exists());
        }
    }

    #[tokio::test]
    async fn formula_uninstall_never_claims_success_while_the_record_remains() {
        let (directory, brew) = migrated_formula_fixture(true);
        let error = brew
            .run(
                vec![Action::Uninstall(legacy_formula())],
                directory.path().into(),
                SharedProgress::default(),
            )
            .await
            .unwrap_err();
        assert!(error.contains("stále eviduje"));
    }

    #[tokio::test]
    async fn details_for_a_removed_formula_use_the_exact_installed_record() {
        let (_directory, brew) = migrated_formula_fixture(false);
        let details = brew.details(&legacy_formula()).await.unwrap();
        assert!(details.from_installation);
        assert_eq!(details.package.key, legacy_formula());
        assert_eq!(details.package.installed_version(), "0.132.1");
    }

    #[tokio::test]
    async fn a_cask_with_the_same_name_never_replaces_a_missing_formula_detail() {
        let (directory, brew) = migrated_formula_fixture(false);
        std::fs::write(directory.path().join("removed"), "").unwrap();
        assert!(brew.details(&legacy_formula()).await.is_err());
    }

    #[tokio::test]
    async fn an_available_definition_does_not_get_the_installed_only_warning() {
        let (directory, brew) = migrated_formula_fixture(false);
        std::fs::write(
            &brew.executable,
            "#!/bin/sh\nbase=${0%/*}\nprintf 'called\\n' >> \"$base/reads\"\ncat \"$base/inventory.json\"\n",
        )
        .unwrap();
        let details = brew.details(&legacy_formula()).await.unwrap();
        assert!(!details.from_installation);
        assert_eq!(details.package.key, legacy_formula());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("reads")).unwrap(),
            "called\n",
        );
    }

    #[test]
    fn command_arguments_are_explicit_and_never_destructive_overrides() {
        assert_eq!(
            Action::Uninstall(sample()).args().unwrap(),
            ["uninstall", "--cask", "team/tap/sample"]
        );
        assert!(
            Action::Install(Key {
                kind: Kind::Cask,
                name: "--force".into()
            })
            .args()
            .is_err()
        );
    }

    fn stale_fixture(app_exists: bool, keep_record: bool) -> (tempfile::TempDir, Brew) {
        let (directory, brew) = fake_brew("");
        let app = directory.path().join("Missing.app");
        if app_exists {
            std::fs::create_dir(&app).unwrap();
        }
        let inventory = serde_json::json!({"formulae":[],"casks":[{"token":"sample","full_token":"team/tap/sample","installed":"1","version":"1","artifacts":[{"app":["Missing.app"],"target":app}]}]});
        std::fs::write(
            directory.path().join("inventory.json"),
            serde_json::to_vec(&inventory).unwrap(),
        )
        .unwrap();
        let script = r#"#!/bin/sh
base=${0%/*}
if [ "$1" = info ]; then
    case " $* " in
      *" --installed "*)
        if [ -f "$base/removed" ]; then printf '{"formulae":[],"casks":[]}'; else cat "$base/inventory.json"; fi ;;
      *) cat "$base/inventory.json" ;;
    esac
elif [ "$1" = uninstall ]; then
    [ "$2" = --cask ] && [ "$3" = --force ] && [ "$4" = team/tap/sample ] || exit 99
    : > "$base/command-ran"
    MARK_REMOVED
else
    exit 88
fi
"#.replace("MARK_REMOVED", if keep_record { ":" } else { ": > \"$base/removed\"" });
        std::fs::write(&brew.executable, script).unwrap();
        (directory, brew)
    }

    #[tokio::test]
    async fn stale_cleanup_uses_homebrew_force_and_checks_that_the_record_disappears() {
        let (directory, brew) = stale_fixture(false, false);
        let result = brew
            .run(
                vec![Action::RemoveStale(sample())],
                directory.path().into(),
                SharedProgress::default(),
            )
            .await;
        assert_eq!(result.unwrap(), 1);
        assert!(directory.path().join("command-ran").exists());
        assert!(
            !Action::RemoveStale(sample())
                .args()
                .unwrap()
                .iter()
                .any(|a| a == "--zap")
        );
    }

    #[tokio::test]
    async fn stale_cleanup_refuses_a_real_app_before_running_any_uninstall() {
        let (directory, brew) = stale_fixture(true, false);
        let result = brew
            .run(
                vec![Action::RemoveStale(sample())],
                directory.path().into(),
                SharedProgress::default(),
            )
            .await;
        assert!(result.unwrap_err().contains("na disku"));
        assert!(!directory.path().join("command-ran").exists());
    }

    #[tokio::test]
    async fn an_exit_zero_with_a_remaining_record_is_not_reported_as_success() {
        let (directory, brew) = stale_fixture(false, true);
        let result = brew
            .run(
                vec![Action::RemoveStale(sample())],
                directory.path().into(),
                SharedProgress::default(),
            )
            .await;
        assert!(result.unwrap_err().contains("stále eviduje"));
    }

    #[tokio::test]
    async fn fake_mutation_receives_exact_args_and_streams_both_outputs() {
        let (directory, brew) =
            fake_brew("printf '%s\\n' \"$@\"\nprintf '\\033[32mready\\033[0m\\n' >&2");
        let progress = SharedProgress::default();
        let result = brew
            .run(
                vec![Action::Install(sample())],
                directory.path().into(),
                progress.clone(),
            )
            .await
            .unwrap();
        assert_eq!(result, 1);
        let logs = progress_snapshot(&progress);
        assert!(logs.lines.iter().any(|l| l == "--cask"));
        assert!(logs.lines.iter().any(|l| l == "team/tap/sample"));
        assert!(logs.lines.iter().any(|l| l == "ready"));
        assert_eq!(logs.completed, 1);
    }

    #[tokio::test]
    async fn a_failed_batch_stops_before_the_next_package_and_reports_failure() {
        let (directory, brew) = fake_brew("echo 'dependency is required' >&2\nexit 17");
        let progress = SharedProgress::default();
        assert!(
            brew.run(
                vec![Action::Uninstall(sample()), Action::Install(sample())],
                directory.path().into(),
                progress.clone()
            )
            .await
            .is_err()
        );
        let logs = progress_snapshot(&progress);
        assert_eq!(logs.completed, 0);
        assert_eq!(logs.lines.iter().filter(|l| l.starts_with("$ ")).count(), 1);
    }

    #[tokio::test]
    async fn a_second_operation_is_rejected_while_the_manager_is_busy() {
        let (directory, brew) = fake_brew("exit 0");
        let _guard = brew.gate.lock().await;
        assert!(
            brew.run(
                vec![Action::Install(sample())],
                directory.path().into(),
                SharedProgress::default()
            )
            .await
            .unwrap_err()
            .contains("inú operáciu")
        );
    }

    #[tokio::test]
    async fn streaming_has_bounded_lines_and_handles_unterminated_output() {
        let (directory, brew) =
            fake_brew("i=0; while [ \"$i\" -lt 450 ]; do echo line; i=$((i+1)); done; printf tail");
        let progress = SharedProgress::default();
        brew.run(
            vec![Action::Install(sample())],
            directory.path().into(),
            progress.clone(),
        )
        .await
        .unwrap();
        let logs = progress_snapshot(&progress);
        assert_eq!(logs.lines.len(), MAX_LOG_LINES);
        assert!(logs.lines.iter().any(|l| l == "tail"));
    }

    #[tokio::test]
    async fn developer_tools_diagnosis_survives_a_truncated_failure_log() {
        let (directory, brew) = fake_brew(
            "echo 'You should download the Command Line Tools for Xcode 27.0.' >&2; i=0; while [ \"$i\" -lt 450 ]; do echo tail >&2; i=$((i+1)); done; exit 1",
        );
        let progress = SharedProgress::default();
        assert!(
            brew.run(
                vec![Action::Upgrade(sample())],
                directory.path().into(),
                progress.clone()
            )
            .await
            .is_err()
        );
        let data = progress_snapshot(&progress);
        assert_eq!(
            data.developer_tools.unwrap().recommended.as_deref(),
            Some("27.0")
        );
        assert!(
            !data
                .lines
                .iter()
                .any(|line| line.contains("should download"))
        );
    }

    #[tokio::test]
    async fn upgrade_all_runs_one_general_brew_command_with_normal_auto_update_policy() {
        let (directory, brew) = fake_brew(
            "[ \"$#\" -eq 1 ] && [ \"$1\" = upgrade ] || exit 99\n[ -z \"${HOMEBREW_NO_AUTO_UPDATE+x}\" ] || exit 98\necho upgraded",
        );
        let progress = SharedProgress::default();
        assert_eq!(
            brew.run(
                vec![Action::UpgradeAll],
                directory.path().into(),
                progress.clone()
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(Action::UpgradeAll.args().unwrap(), ["upgrade"]);
        assert!(Action::UpgradeAll.key().is_none());
        let lines = progress_snapshot(&progress).lines;
        assert_eq!(
            lines.iter().filter(|line| line.starts_with("$ ")).count(),
            1
        );
        assert!(lines.iter().any(|line| line == "$ brew upgrade"));
    }
}
