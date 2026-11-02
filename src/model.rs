use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Formula,
    Cask,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Formula => crate::i18n::t("Nástroj"),
            Self::Cask => crate::i18n::t("Aplikácia"),
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Self::Formula => "--formula",
            Self::Cask => "--cask",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Key {
    pub kind: Kind,
    pub name: String,
}

impl Key {
    pub fn validate(&self) -> Result<(), String> {
        let parts: Vec<_> = self.name.split('/').collect();
        if !matches!(parts.len(), 1 | 3)
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part.as_bytes()[0].is_ascii_alphanumeric()
                    || !part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.+@".contains(&c))
                    || *part == "."
                    || *part == ".."
            })
        {
            return Err(crate::i18n::t("Neplatný identifikátor balíčka Homebrew.").into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub versions: Vec<String>,
    pub on_request: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub key: Key,
    pub token: String,
    pub title: String,
    pub description: String,
    pub homepage: String,
    pub version: String,
    pub tap: String,
    pub aliases: Vec<String>,
    pub dependencies: Vec<Key>,
    #[serde(default)]
    pub minimum_macos: Option<String>,
    #[serde(default)]
    pub runtime_dependencies: Vec<Key>,
    #[serde(default)]
    pub app_paths: Vec<String>,
    pub auto_updates: bool,
    pub disabled: bool,
    pub deprecated: bool,
    pub caveats: String,
    pub installed: Option<Installation>,
    #[serde(skip)]
    pub outdated: bool,
    #[serde(skip)]
    search_index: String,
    #[serde(skip)]
    search_token: String,
    #[serde(skip)]
    search_title: String,
}

pub struct Query {
    whole: String,
    words: Vec<String>,
}

impl Query {
    pub fn new(query: &str) -> Self {
        let whole = query.trim().to_lowercase();
        let words = whole.split_whitespace().map(str::to_owned).collect();
        Self { whole, words }
    }
}

impl Package {
    pub fn reindex(&mut self) {
        self.search_token = self.token.to_lowercase();
        self.search_title = self.title.to_lowercase();
        self.search_index = format!(
            "{} {} {} {} {}",
            self.key.name,
            self.token,
            self.title,
            self.description,
            self.aliases.join(" ")
        )
        .to_lowercase();
    }

    pub fn search_score(&self, query: &str) -> Option<u8> {
        self.score(&Query::new(query))
    }

    pub fn score(&self, prepared: &Query) -> Option<u8> {
        let query = &prepared.whole;
        if !prepared
            .words
            .iter()
            .all(|word| self.search_index.contains(word.as_str()))
        {
            return None;
        }
        Some(
            if query.is_empty() || self.token.eq_ignore_ascii_case(query) {
                0
            } else if self.search_token.starts_with(query) || self.search_title.starts_with(query) {
                1
            } else if self.search_token.contains(query) {
                2
            } else {
                3
            },
        )
    }

    pub fn installed_version(&self) -> String {
        self.installed
            .as_ref()
            .map(|installed| installed.versions.join(", "))
            .unwrap_or_default()
    }

    pub fn is_pinned(&self) -> bool {
        self.installed.as_ref().is_some_and(|i| i.pinned)
    }
}

#[derive(Debug, Deserialize)]
struct Formula {
    name: String,
    #[serde(default, deserialize_with = "null_default")]
    full_name: String,
    #[serde(default, deserialize_with = "null_default")]
    desc: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    homepage: String,
    #[serde(default, deserialize_with = "null_default")]
    tap: String,
    #[serde(default, deserialize_with = "null_default")]
    versions: Versions,
    #[serde(default, deserialize_with = "null_default")]
    aliases: Vec<String>,
    #[serde(default, deserialize_with = "null_default")]
    dependencies: Vec<String>,
    #[serde(default, deserialize_with = "null_default")]
    installed: Vec<Receipt>,
    #[serde(default, deserialize_with = "null_default")]
    pinned: bool,
    #[serde(default, deserialize_with = "null_default")]
    disabled: bool,
    #[serde(default, deserialize_with = "null_default")]
    deprecated: bool,
    #[serde(default, deserialize_with = "null_default")]
    caveats: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Versions {
    stable: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Receipt {
    version: String,
    #[serde(default, deserialize_with = "null_default")]
    installed_on_request: bool,
    #[serde(default, deserialize_with = "null_default")]
    runtime_dependencies: Option<Vec<RuntimeDependency>>,
}

#[derive(Debug, Deserialize)]
struct RuntimeDependency {
    full_name: String,
    #[serde(default)]
    declared_directly: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Cask {
    token: String,
    #[serde(default, deserialize_with = "null_default")]
    artifacts: Vec<Value>,
    #[serde(default, deserialize_with = "null_default")]
    full_token: String,
    #[serde(default, deserialize_with = "null_default")]
    name: Vec<String>,
    #[serde(default, deserialize_with = "null_default")]
    desc: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    homepage: String,
    #[serde(default, deserialize_with = "null_default")]
    tap: String,
    #[serde(default, deserialize_with = "null_default")]
    version: String,
    #[serde(default, deserialize_with = "null_default")]
    installed: Option<Value>,
    #[serde(default, deserialize_with = "null_default")]
    depends_on: Value,
    #[serde(default, deserialize_with = "null_default")]
    auto_updates: bool,
    #[serde(default, deserialize_with = "null_default")]
    pinned: bool,
    #[serde(default, deserialize_with = "null_default")]
    disabled: bool,
    #[serde(default, deserialize_with = "null_default")]
    deprecated: bool,
    #[serde(default, deserialize_with = "null_default")]
    caveats: Option<String>,
}

impl From<Formula> for Package {
    fn from(raw: Formula) -> Self {
        let description = raw.desc.unwrap_or_default();
        let installed = (!raw.installed.is_empty()).then(|| Installation {
            versions: raw.installed.iter().map(|r| r.version.clone()).collect(),
            on_request: raw.installed.iter().any(|r| r.installed_on_request),
            pinned: raw.pinned,
        });
        let has_runtime_receipt = raw
            .installed
            .iter()
            .any(|r| r.runtime_dependencies.is_some());
        let mut dependencies = Vec::new();
        let mut runtime_dependencies = Vec::new();
        if has_runtime_receipt {
            for dependency in raw
                .installed
                .iter()
                .filter_map(|r| r.runtime_dependencies.as_ref())
                .flatten()
            {
                let key = Key {
                    kind: Kind::Formula,
                    name: dependency.full_name.clone(),
                };
                if dependency.declared_directly == Some(true) {
                    dependencies.push(key);
                } else {
                    runtime_dependencies.push(key);
                }
            }
        } else {
            dependencies = raw
                .dependencies
                .into_iter()
                .map(|name| Key {
                    kind: Kind::Formula,
                    name,
                })
                .collect();
        }
        dependencies.sort();
        dependencies.dedup();
        runtime_dependencies.sort();
        runtime_dependencies.dedup();
        runtime_dependencies.retain(|key| !dependencies.contains(key));
        let mut package = Self {
            key: Key {
                kind: Kind::Formula,
                name: if raw.full_name.is_empty() {
                    raw.name.clone()
                } else {
                    raw.full_name
                },
            },
            title: raw.name.clone(),
            token: raw.name,
            description,
            homepage: raw.homepage,
            version: raw.versions.stable.unwrap_or_else(|| "HEAD".into()),
            tap: raw.tap,
            aliases: raw.aliases,
            dependencies,
            minimum_macos: None,
            runtime_dependencies,
            app_paths: Vec::new(),
            auto_updates: false,
            disabled: raw.disabled,
            deprecated: raw.deprecated,
            caveats: raw.caveats.unwrap_or_default(),
            installed,
            outdated: false,
            search_index: String::new(),
            search_token: String::new(),
            search_title: String::new(),
        };
        package.reindex();
        package
    }
}

impl From<Cask> for Package {
    fn from(raw: Cask) -> Self {
        let mut app_paths = Vec::new();
        for artifact in &raw.artifacts {
            if let Some(app) = artifact.get("app").and_then(Value::as_array) {
                let target = artifact
                    .get("target")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        app.iter()
                            .find_map(|v| v.get("target").and_then(Value::as_str))
                    })
                    .or_else(|| app.first().and_then(Value::as_str));
                if let Some(target) = target.filter(|p| p.ends_with(".app")) {
                    app_paths.push(target.to_owned());
                }
            }
            if let Some(uninstall) = artifact.get("uninstall").and_then(Value::as_array) {
                for item in uninstall {
                    let paths = item.get("delete").and_then(Value::as_array);
                    if let Some(paths) = paths {
                        app_paths.extend(
                            paths
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|p| {
                                    p.starts_with("/Applications/")
                                        && p.ends_with(".app")
                                        && !p.contains('*')
                                })
                                .map(str::to_owned),
                        );
                    }
                }
            }
        }
        app_paths.sort();
        app_paths.dedup();
        let description = raw.desc.unwrap_or_default();
        let versions = match raw.installed {
            Some(Value::String(version)) if !version.is_empty() => vec![version],
            Some(Value::Array(versions)) => versions
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        let installed = (!versions.is_empty()).then_some(Installation {
            versions,
            on_request: true,
            pinned: raw.pinned,
        });
        let mut dependencies = Vec::new();
        for (field, kind) in [("formula", Kind::Formula), ("cask", Kind::Cask)] {
            if let Some(values) = raw.depends_on.get(field) {
                let names: Vec<_> = match values {
                    Value::String(name) => vec![name.as_str()],
                    Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
                    _ => vec![],
                };
                dependencies.extend(names.into_iter().map(|name| Key {
                    kind,
                    name: name.to_owned(),
                }));
            }
        }
        dependencies.sort();
        dependencies.dedup();
        let mut package = Self {
            key: Key {
                kind: Kind::Cask,
                name: if raw.full_token.is_empty() {
                    raw.token.clone()
                } else {
                    raw.full_token
                },
            },
            title: raw
                .name
                .first()
                .cloned()
                .unwrap_or_else(|| raw.token.clone()),
            token: raw.token,
            description,
            homepage: raw.homepage,
            version: raw.version,
            tap: raw.tap,
            aliases: raw.name,
            dependencies,
            minimum_macos: raw
                .depends_on
                .get("macos")
                .and_then(|value| value.get(">="))
                .and_then(Value::as_array)
                .and_then(|versions| versions.first())
                .and_then(Value::as_str)
                .map(str::to_owned),
            runtime_dependencies: vec![],
            app_paths,
            auto_updates: raw.auto_updates,
            disabled: raw.disabled,
            deprecated: raw.deprecated,
            caveats: raw.caveats.unwrap_or_default(),
            installed,
            outdated: false,
            search_index: String::new(),
            search_token: String::new(),
            search_title: String::new(),
        };
        package.reindex();
        package
    }
}

pub fn parse_catalog(bytes: &[u8], kind: Kind) -> Result<Vec<Package>, String> {
    let packages: Vec<Package> = match kind {
        Kind::Formula => serde_json::from_slice::<Vec<Formula>>(bytes)
            .map_err(|e| {
                crate::i18n::message(
                    "Neplatný katalóg nástrojov: {e}",
                    &[],
                    &[("e", e.to_string())],
                )
            })?
            .into_iter()
            .map(Into::into)
            .collect(),
        Kind::Cask => serde_json::from_slice::<Vec<Cask>>(bytes)
            .map_err(|e| {
                crate::i18n::message(
                    "Neplatný katalóg aplikácií: {e}",
                    &[],
                    &[("e", e.to_string())],
                )
            })?
            .into_iter()
            .map(Into::into)
            .collect(),
    };
    for package in &packages {
        package.key.validate()?;
    }
    Ok(packages)
}

pub fn parse_inventory(bytes: &[u8]) -> Result<Vec<Package>, String> {
    #[derive(Deserialize)]
    struct Inventory {
        formulae: Vec<Formula>,
        casks: Vec<Cask>,
    }
    let inventory: Inventory = serde_json::from_slice(bytes).map_err(|e| {
        crate::i18n::message(
            "Homebrew vrátil neplatný inventár: {e}",
            &[],
            &[("e", e.to_string())],
        )
    })?;
    let packages: Vec<Package> = inventory
        .formulae
        .into_iter()
        .map(Into::into)
        .chain(inventory.casks.into_iter().map(Into::into))
        .collect();
    for package in &packages {
        package.key.validate()?;
    }
    Ok(packages)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outdated {
    pub key: Key,
    pub current_version: String,
    pub pinned: bool,
}

pub fn parse_outdated(bytes: &[u8]) -> Result<Vec<Outdated>, String> {
    #[derive(Deserialize)]
    struct Entry {
        name: String,
        current_version: String,
        #[serde(default, deserialize_with = "null_default")]
        pinned: bool,
    }
    #[derive(Deserialize)]
    struct List {
        formulae: Vec<Entry>,
        casks: Vec<Entry>,
    }
    let list: List = serde_json::from_slice(bytes).map_err(|e| {
        crate::i18n::message(
            "Homebrew vrátil neplatný zoznam aktualizácií: {e}",
            &[],
            &[("e", e.to_string())],
        )
    })?;
    Ok(list
        .formulae
        .into_iter()
        .map(|e| (Kind::Formula, e))
        .chain(list.casks.into_iter().map(|e| (Kind::Cask, e)))
        .map(|(kind, e)| Outdated {
            key: Key { kind, name: e.name },
            current_version: e.current_version,
            pinned: e.pinned,
        })
        .collect())
}

pub fn merge(catalog: &[Package], inventory: &[Package], outdated: &[Outdated]) -> Vec<Package> {
    let mut result: BTreeMap<Key, Package> = catalog
        .iter()
        .cloned()
        .map(|mut p| {
            p.installed = None;
            p.outdated = false;
            (p.key.clone(), p)
        })
        .collect();
    for installed in inventory {
        result
            .entry(installed.key.clone())
            .and_modify(|p| {
                p.installed = installed.installed.clone();
                p.version = installed.version.clone();
                p.dependencies = installed.dependencies.clone();
                p.minimum_macos = installed.minimum_macos.clone();
                p.runtime_dependencies = installed.runtime_dependencies.clone();
                if !installed.caveats.is_empty() {
                    p.caveats = installed.caveats.clone();
                }
            })
            .or_insert_with(|| installed.clone());
    }
    for update in outdated {
        // `outdated` may use a short token even for a third-party tap. Only
        // resolve that fallback when there is exactly one installed match.
        let key = if result
            .get(&update.key)
            .is_some_and(|p| p.installed.is_some())
        {
            Some(update.key.clone())
        } else {
            let mut matches = result.values().filter(|p| {
                p.installed.is_some() && p.key.kind == update.key.kind && p.token == update.key.name
            });
            let first = matches.next().map(|p| p.key.clone());
            if matches.next().is_none() {
                first
            } else {
                None
            }
        };
        if let Some(package) = key.and_then(|key| result.get_mut(&key)) {
            package.outdated = package.installed.is_some();
            package.version = update.current_version.clone();
            if let Some(installed) = &mut package.installed {
                installed.pinned |= update.pinned;
            }
        }
    }
    let mut packages: Vec<_> = result.into_values().collect();
    packages.sort_by_cached_key(|p| (p.title.to_lowercase(), p.key.clone()));
    packages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_tap_and_app_artifacts_match_real_cask_inventory() {
        let packages = parse_inventory(br#"{"formulae":[],"casks":[{"token":"editor","full_token":null,"tap":null,"homepage":null,"installed":"1","version":"2","artifacts":[{"app":["Editor.app"],"target":"/Applications/Editor.app"}]}]}"#).unwrap();
        assert_eq!(packages[0].installed_version(), "1");
        assert_eq!(packages[0].app_paths, ["/Applications/Editor.app"]);
        assert!(packages[0].tap.is_empty());
    }

    #[test]
    fn nullable_homebrew_flags_are_unknown_or_false_not_a_failed_inventory() {
        let packages = parse_inventory(br#"{"formulae":[{"name":"sample","pinned":null,"disabled":null,"deprecated":null,"installed":[{"version":"1","installed_on_request":null}]}],"casks":[{"token":"app","auto_updates":null,"pinned":null,"disabled":null,"deprecated":null,"installed":"1"}]}"#).unwrap();
        assert_eq!(packages.len(), 2);
        assert!(!packages[0].is_pinned());
        assert!(!packages[1].auto_updates);
        let updates = parse_outdated(
            br#"{"formulae":[{"name":"sample","current_version":"2","pinned":null}],"casks":[]}"#,
        )
        .unwrap();
        assert!(!updates[0].pinned);
    }

    #[test]
    fn parses_real_shapes_and_request_receipts() {
        let packages = parse_inventory(br#"{"formulae":[{"name":"tool","full_name":"team/tap/tool","versions":{"stable":"2.0"},"installed":[{"version":"1.0","installed_on_request":true,"runtime_dependencies":[{"full_name":"openssl@3","declared_directly":true}]}]}],"casks":[{"token":"sample","name":["Sample App"],"version":"latest","installed":"3.2","auto_updates":true}]}"#).unwrap();
        assert_eq!(packages[0].key.name, "team/tap/tool");
        assert_eq!(packages[0].installed_version(), "1.0");
        assert!(packages[0].installed.as_ref().unwrap().on_request);
        assert_eq!(
            packages[0].dependencies,
            [Key {
                kind: Kind::Formula,
                name: "openssl@3".into()
            }]
        );
        assert_eq!(packages[1].installed_version(), "3.2");
        assert!(packages[1].auto_updates);
    }

    #[test]
    fn search_covers_descriptions_aliases_and_all_words() {
        let package = parse_catalog(br#"[{"token":"visual-studio-code","name":["Visual Studio Code","VSCode"],"desc":"Code editor for developers","version":"1"}]"#, Kind::Cask).unwrap().remove(0);
        assert!(package.search_score("VSCODE").is_some());
        assert!(package.search_score("editor code").is_some());
        assert!(package.search_score("editor banana").is_none());
        assert_eq!(package.search_score("visual-studio-code"), Some(0));
    }

    #[test]
    fn merge_preserves_type_and_tap_identity_and_clears_removed_installs() {
        let catalog = parse_catalog(
            br#"[{"name":"sample","versions":{"stable":"2"}}]"#,
            Kind::Formula,
        )
        .unwrap();
        let inventory = parse_inventory(br#"{"formulae":[{"name":"sample","full_name":"team/tap/sample","installed":[{"version":"1"}]}],"casks":[{"token":"sample","installed":"1","version":"2"}]}"#).unwrap();
        let merged = merge(&catalog, &inventory, &[]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.iter().filter(|p| p.installed.is_some()).count(), 2);
        let empty = merge(&merged, &[], &[]);
        assert!(empty.iter().all(|p| p.installed.is_none()));
    }

    #[test]
    fn updates_use_brew_versions_not_string_comparison_and_keep_pins() {
        let inventory = parse_inventory(br#"{"formulae":[],"casks":[{"token":"sample","version":"latest","installed":"9","pinned":true}]}"#).unwrap();
        let updates = parse_outdated(
            br#"{"formulae":[],"casks":[{"name":"sample","current_version":"10"}]}"#,
        )
        .unwrap();
        let merged = merge(&[], &inventory, &updates);
        assert!(merged[0].outdated);
        assert!(merged[0].is_pinned());
        assert_eq!(merged[0].version, "10");
    }

    #[test]
    fn short_update_token_resolves_to_the_installed_tap_not_an_uninstalled_public_entry() {
        let catalog = parse_catalog(
            br#"[{"name":"sample","versions":{"stable":"2"}}]"#,
            Kind::Formula,
        )
        .unwrap();
        let inventory = parse_inventory(br#"{"formulae":[{"name":"sample","full_name":"team/tap/sample","installed":[{"version":"1"}]}],"casks":[]}"#).unwrap();
        let outdated =
            parse_outdated(br#"{"formulae":[{"name":"sample","current_version":"3"}],"casks":[]}"#)
                .unwrap();
        let packages = merge(&catalog, &inventory, &outdated);
        assert!(
            packages
                .iter()
                .find(|p| p.key.name == "team/tap/sample")
                .unwrap()
                .outdated
        );
        assert!(
            !packages
                .iter()
                .find(|p| p.key.name == "sample")
                .unwrap()
                .outdated
        );
    }

    #[test]
    fn installed_versions_and_os_requirements_follow_the_active_brew_metadata() {
        let catalog = parse_catalog(
            br#"[{"token":"sample","version":"3","depends_on":{"macos":{">=":["28"]}}}]"#,
            Kind::Cask,
        )
        .unwrap();
        let inventory = parse_inventory(br#"{"formulae":[],"casks":[{"token":"sample","version":"2","installed":"2","depends_on":{"macos":{">=":["13"]}}}]}"#).unwrap();
        let packages = merge(&catalog, &inventory, &[]);
        assert_eq!(packages[0].version, "2");
        assert_eq!(packages[0].minimum_macos.as_deref(), Some("13"));
        assert!(!packages[0].outdated);
    }

    #[test]
    fn rejects_option_paths_urls_and_shell_input() {
        for name in [
            "--help",
            "../tool",
            "/tmp/evil.rb",
            "https://evil/tool",
            "tool;id",
            "tool\n",
            "a/b",
            "a/../tool",
        ] {
            assert!(
                Key {
                    kind: Kind::Formula,
                    name: name.into()
                }
                .validate()
                .is_err(),
                "{name}"
            );
        }
        for name in ["openssl@3", "team/tap/tool", "libstdc++", "3proxy"] {
            assert!(
                Key {
                    kind: Kind::Formula,
                    name: name.into()
                }
                .validate()
                .is_ok(),
                "{name}"
            );
        }
    }
}
