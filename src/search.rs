use crate::app::{Page, Scope};
use crate::model::{Kind, Package, Query};

pub fn find(packages: &[Package], page: Page, scope: Scope, query: &str) -> Vec<usize> {
    if page == Page::Search && query.trim().is_empty() {
        return vec![];
    }
    let query = Query::new(query);
    let mut scored: Vec<_> = packages
        .iter()
        .enumerate()
        .filter_map(|(index, package)| {
            if page == Page::Search && package.disabled {
                return None;
            }
            if page == Page::Installed && package.installed.is_none() {
                return None;
            }
            if !match scope {
                Scope::All => true,
                Scope::Apps => package.key.kind == Kind::Cask,
                Scope::Tools => package.key.kind == Kind::Formula,
            } {
                return None;
            }
            package.score(&query).map(|score| (score, index))
        })
        .collect();
    scored.sort_by_key(|&(score, _)| score);
    scored.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_catalog;
    #[test]
    fn search_hides_disabled_packages_but_inventory_keeps_them() {
        let packages = crate::model::parse_inventory(br#"{"formulae":[],"casks":[{"token":"editor-old","disabled":true,"installed":"1"},{"token":"editor-current","version":"2"}]}"#).unwrap();
        assert_eq!(find(&packages, Page::Search, Scope::All, "editor"), [1]);
        assert_eq!(find(&packages, Page::Installed, Scope::All, "editor"), [0]);
    }

    #[test]
    fn prepared_search_ranks_exact_names_then_prefixes_and_matches_all_words() {
        let packages = parse_catalog(br#"[{"token":"editor-tools","desc":"editor helper"},{"token":"editor","desc":"source code"},{"token":"other","desc":"source editor code"}]"#, Kind::Cask).unwrap();
        assert_eq!(
            find(&packages, Page::Search, Scope::All, "EDITOR"),
            [1, 0, 2]
        );
        assert_eq!(
            find(&packages, Page::Search, Scope::All, "code source"),
            [1, 2]
        );
    }
}
