use crate::model::{Kind, Package};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Default)]
pub struct Visual {
    pub icon: Option<PathBuf>,
    pub bytes: Option<u64>,
    pub app_missing: bool,
    pub app_bundle: Option<PathBuf>,
}

pub fn format_size(bytes: u64) -> String {
    let value = bytes as f64;
    let formatted = if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", value / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", value / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", value / (1024.0 * 1024.0 * 1024.0))
    };
    crate::i18n::decimal(formatted)
}

pub fn app_candidates(declarations: &[String], home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for declaration in declarations {
        let path = PathBuf::from(declaration);
        if path.extension().is_none_or(|ext| ext != "app") {
            continue;
        }
        if path.is_absolute() {
            paths.push(path);
        } else if path.components().count() == 1 {
            paths.push(Path::new("/Applications").join(&path));
            paths.push(home.join("Applications").join(&path));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

async fn size(paths: &[PathBuf]) -> Option<u64> {
    if paths.is_empty() {
        return None;
    }
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("/usr/bin/du")
            .arg("-sk")
            .args(paths)
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
    let mut total = 0_u64;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let kb: u64 = line.split_whitespace().next()?.parse().ok()?;
        total = total.checked_add(kb.checked_mul(1024)?)?;
    }
    Some(total)
}

async fn icon_source(bundle: &Path) -> Option<PathBuf> {
    let resources = bundle.join("Contents/Resources");
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
            .arg(bundle.join("Contents/Info.plist"))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let file = Path::new(&value);
        // A bundle's icon name is a filename, never an arbitrary path.
        if file.components().count() == 1 {
            let name = if file.extension().is_some() {
                value
            } else {
                format!("{value}.icns")
            };
            let path = resources.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    let mut entries = tokio::fs::read_dir(&resources).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "icns") {
            return Some(path);
        }
    }
    None
}

async fn icon(bundle: &Path, package: &Package, directory: &Path) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    package.key.name.hash(&mut hasher);
    package.installed_version().hash(&mut hasher);
    let directory = directory.join("icons");
    tokio::fs::create_dir_all(&directory).await.ok()?;
    let path = directory.join(format!("{:016x}.png", hasher.finish()));
    if path.is_file() {
        return Some(path);
    }
    let source = icon_source(bundle).await?;
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("/usr/bin/sips")
            .args(["-s", "format", "png", "--resampleHeightWidthMax", "128"])
            .arg(source)
            .arg("--out")
            .arg(&path)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if output.status.success() && path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub async fn inspect(
    package: Package,
    cellar: PathBuf,
    directory: PathBuf,
    gate: Arc<tokio::sync::Semaphore>,
) -> Visual {
    let Ok(_permit) = gate.acquire().await else {
        return Visual::default();
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut paths = match package.key.kind {
        Kind::Formula => vec![cellar.join(&package.token)],
        Kind::Cask => app_candidates(&package.app_paths, &home),
    };
    let app_missing = !package.app_paths.is_empty()
        && !paths.is_empty()
        && paths
            .iter()
            .all(|path| matches!(path.try_exists(), Ok(false)));
    paths.retain(|path| path.is_dir());
    let bundle = paths
        .iter()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"));
    let (bytes, icon) = tokio::join!(size(&paths), async {
        if let Some(bundle) = bundle {
            icon(bundle, &package, &directory).await
        } else {
            None
        }
    });
    Visual {
        bytes,
        icon,
        app_missing,
        app_bundle: bundle.cloned(),
    }
}

pub async fn open_application(package: &Package, bundle: &Path) -> Result<(), String> {
    open_with(package, bundle, Path::new("/usr/bin/open")).await
}

async fn open_with(package: &Package, bundle: &Path, opener: &Path) -> Result<(), String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if package.key.kind != Kind::Cask
        || package.installed.is_none()
        || !app_candidates(&package.app_paths, &home)
            .iter()
            .any(|candidate| candidate == bundle)
        || bundle
            .extension()
            .is_none_or(|extension| extension != "app")
        || !bundle.is_absolute()
        || !bundle.is_dir()
    {
        return Err(
            crate::i18n::t("Aplikácia už nie je na deklarovanom mieste. Obnov údaje.").into(),
        );
    }
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new(opener)
            .arg("-a")
            .arg(bundle)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| crate::i18n::t("Otvorenie aplikácie neodpovedalo do 10 sekúnd.").to_owned())?
    .map_err(|e| {
        crate::i18n::message(
            "Aplikáciu sa nepodarilo otvoriť: {e}",
            &[],
            &[("e", e.to_string())],
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(
            crate::i18n::t("macOS aplikáciu neotvoril. Skontroluj, či je stále nainštalovaná.")
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_app_bundles_and_safe_relative_names_become_candidates() {
        let paths = app_candidates(
            &[
                "Editor.app".into(),
                "../../Secrets.app".into(),
                "/Applications/Actual.app".into(),
                "/".into(),
            ],
            Path::new("/Users/example"),
        );
        assert_eq!(paths.len(), 3);
        assert!(paths.contains(&PathBuf::from("/Applications/Editor.app")));
        assert!(paths.contains(&PathBuf::from("/Users/example/Applications/Editor.app")));
    }

    #[tokio::test]
    async fn disk_size_is_measured_from_the_real_fixture_directory() {
        let directory = tempfile::tempdir().unwrap();
        tokio::fs::write(directory.path().join("data"), vec![7_u8; 64 * 1024])
            .await
            .unwrap();
        assert!(size(&[directory.path().to_owned()]).await.unwrap() >= 64 * 1024);
        assert_eq!(size(&[]).await, None);
    }

    #[tokio::test]
    async fn opens_only_an_existing_declared_bundle_with_literal_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("Example App.app");
        std::fs::create_dir(&bundle).unwrap();
        let executable = directory.path().join("open");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"${0%/*}/arguments\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let inventory = serde_json::json!({"formulae":[],"casks":[{"token":"example","installed":"1","version":"1","artifacts":[{"app":["Example App.app"],"target":bundle}]}]});
        let package = crate::model::parse_inventory(&serde_json::to_vec(&inventory).unwrap())
            .unwrap()
            .remove(0);
        open_with(&package, &bundle, &executable).await.unwrap();
        let args = std::fs::read_to_string(directory.path().join("arguments")).unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            ["-a", bundle.to_str().unwrap()]
        );
        std::fs::remove_file(directory.path().join("arguments")).unwrap();
        let other = directory.path().join("Unrelated.app");
        std::fs::create_dir(&other).unwrap();
        assert!(open_with(&package, &other, &executable).await.is_err());
        std::fs::remove_dir(&bundle).unwrap();
        assert!(open_with(&package, &bundle, &executable).await.is_err());
        assert!(!directory.path().join("arguments").exists());
    }
}
