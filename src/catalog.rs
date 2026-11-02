use crate::model::{Kind, Package, parse_catalog};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CACHE_SCHEMA: u32 = 4;
const MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const API: &str = "https://formulae.brew.sh/api";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    schema: u32,
    pub fetched_at: u64,
    pub packages: Vec<Package>,
}

pub fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("Library/Caches/local.brew-store")
}

pub fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn read_cache(directory: &Path) -> Result<Option<Catalog>, String> {
    let bytes = match tokio::fs::read(directory.join("catalog-v4.json")).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(crate::i18n::message(
                "Uložený katalóg sa nedá prečítať: {e}",
                &[],
                &[("e", e.to_string())],
            ));
        }
    };
    let mut catalog: Catalog = serde_json::from_slice(&bytes).map_err(|e| {
        crate::i18n::message(
            "Uložený katalóg je poškodený: {e}",
            &[],
            &[("e", e.to_string())],
        )
    })?;
    if catalog.schema != CACHE_SCHEMA || catalog.packages.is_empty() {
        return Err(
            crate::i18n::t("Uložený katalóg má nepodporovaný formát alebo je prázdny.").into(),
        );
    }
    for package in &mut catalog.packages {
        package.key.validate()?;
        // The cache is public metadata only, never a source of local install state.
        package.installed = None;
        package.outdated = false;
        package.reindex();
    }
    Ok(Some(catalog))
}

async fn write_cache(directory: &Path, catalog: &Catalog) -> Result<(), String> {
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|e| e.to_string())?;
    let temp = directory.join(format!("catalog-v4.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(catalog).map_err(|e| e.to_string())?;
    tokio::fs::write(&temp, bytes)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(&temp, directory.join("catalog-v4.json"))
        .await
        .map_err(|e| e.to_string())
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            crate::i18n::message("Katalóg je nedostupný: {e}", &[], &[("e", e.to_string())])
        })?
        .error_for_status()
        .map_err(|e| crate::i18n::message("Server katalógu: {e}", &[], &[("e", e.to_string())]))?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(crate::i18n::t("Katalóg prekročil povolenú veľkosť.").into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub async fn refresh(directory: PathBuf) -> Result<(Catalog, Option<String>), String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("BrewStore/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let formula_url = format!("{API}/formula.json");
    let cask_url = format!("{API}/cask.json");
    let (formulae, casks) = tokio::try_join!(
        download(&client, &formula_url),
        download(&client, &cask_url),
    )?;
    let catalog = tokio::task::spawn_blocking(move || {
        let mut packages = parse_catalog(&formulae, Kind::Formula)?;
        packages.extend(parse_catalog(&casks, Kind::Cask)?);
        if packages.is_empty() {
            return Err(crate::i18n::t("Homebrew vrátil prázdny katalóg.").to_owned());
        }
        Ok(Catalog {
            schema: CACHE_SCHEMA,
            fetched_at: timestamp(),
            packages,
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let warning = write_cache(&directory, &catalog).await.err().map(|e| {
        crate::i18n::message(
            "Katalóg je načítaný, ale nepodarilo sa ho uložiť: {e}",
            &[],
            &[("e", e.to_string())],
        )
    });
    Ok((catalog, warning))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_round_trip_restores_search_and_never_restores_installation() {
        let directory = tempfile::tempdir().unwrap();
        let packages = parse_catalog(
            br#"[{"token":"sample","name":["Sample Editor"],"version":"1","installed":"1"}]"#,
            Kind::Cask,
        )
        .unwrap();
        let catalog = Catalog {
            schema: CACHE_SCHEMA,
            fetched_at: 123,
            packages,
        };
        write_cache(directory.path(), &catalog).await.unwrap();
        let cached = read_cache(directory.path()).await.unwrap().unwrap();
        assert_eq!(cached.fetched_at, 123);
        assert!(cached.packages[0].search_score("editor").is_some());
        assert!(cached.packages[0].installed.is_none());
    }

    #[tokio::test]
    async fn missing_or_corrupt_cache_does_not_look_like_an_empty_success() {
        let directory = tempfile::tempdir().unwrap();
        assert!(read_cache(directory.path()).await.unwrap().is_none());
        tokio::fs::write(directory.path().join("catalog-v4.json"), b"{broken")
            .await
            .unwrap();
        assert!(read_cache(directory.path()).await.is_err());
    }
}
