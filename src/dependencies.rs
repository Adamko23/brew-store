use crate::model::{Key, Kind, Package};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Default)]
pub struct InstalledGraph {
    needs: BTreeMap<Key, Vec<Key>>,
    users: BTreeMap<Key, Vec<Key>>,
    runtime_needs: BTreeMap<Key, Vec<Key>>,
    runtime_users: BTreeMap<Key, Vec<Key>>,
    indirect: BTreeMap<Key, Vec<Key>>,
    unresolved: BTreeMap<Key, Vec<Key>>,
}

impl InstalledGraph {
    pub fn build(inventory: &[Package]) -> Self {
        let installed: BTreeMap<_, _> = inventory
            .iter()
            .filter(|p| p.installed.is_some())
            .map(|p| (p.key.clone(), p))
            .collect();
        let mut graph = Self::default();
        for source in installed.values() {
            for (dependency, direct) in source
                .dependencies
                .iter()
                .map(|key| (key, true))
                .chain(source.runtime_dependencies.iter().map(|key| (key, false)))
            {
                let canonical = if installed.contains_key(dependency) {
                    Some(dependency.clone())
                } else {
                    let prefix = if dependency.kind == Kind::Formula {
                        "homebrew/core/"
                    } else {
                        "homebrew/cask/"
                    };
                    let token = dependency
                        .name
                        .strip_prefix(prefix)
                        .unwrap_or(&dependency.name);
                    let mut matches = installed.values().filter(|p| {
                        p.key.kind == dependency.kind
                            && (p.token == token || p.aliases.iter().any(|alias| alias == token))
                    });
                    let first = matches.next().map(|p| p.key.clone());
                    if matches.next().is_none() {
                        first
                    } else {
                        None
                    }
                };
                if let Some(target) = canonical {
                    let (needs, users) = if direct {
                        (&mut graph.needs, &mut graph.users)
                    } else {
                        (&mut graph.runtime_needs, &mut graph.runtime_users)
                    };
                    needs
                        .entry(source.key.clone())
                        .or_default()
                        .push(target.clone());
                    users.entry(target).or_default().push(source.key.clone());
                } else {
                    graph
                        .unresolved
                        .entry(source.key.clone())
                        .or_default()
                        .push(dependency.clone());
                }
            }
        }
        for values in graph
            .needs
            .values_mut()
            .chain(graph.users.values_mut())
            .chain(graph.runtime_needs.values_mut())
            .chain(graph.runtime_users.values_mut())
            .chain(graph.unresolved.values_mut())
        {
            values.sort();
            values.dedup();
        }
        for key in installed.keys() {
            let mut seen = BTreeSet::from([key.clone()]);
            let mut queue: VecDeque<_> = graph
                .direct_users(key)
                .iter()
                .chain(graph.runtime_users(key))
                .cloned()
                .collect();
            let mut indirect = Vec::new();
            while let Some(user) = queue.pop_front() {
                if !seen.insert(user.clone()) {
                    continue;
                }
                if !graph.direct_users(key).contains(&user)
                    && !graph.runtime_users(key).contains(&user)
                {
                    indirect.push(user.clone());
                }
                queue.extend(graph.direct_users(&user).iter().cloned());
                queue.extend(graph.runtime_users(&user).iter().cloned());
            }
            indirect.sort();
            graph.indirect.insert(key.clone(), indirect);
        }
        graph
    }

    pub fn direct_users(&self, key: &Key) -> &[Key] {
        self.users.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn indirect_users(&self, key: &Key) -> &[Key] {
        self.indirect.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn runtime_users(&self, key: &Key) -> &[Key] {
        self.runtime_users
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn runtime_needs(&self, key: &Key) -> &[Key] {
        self.runtime_needs
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn needs(&self, key: &Key) -> &[Key] {
        self.needs.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn unresolved(&self, key: &Key) -> &[Key] {
        self.unresolved.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_inventory;

    fn key(kind: Kind, name: &str) -> Key {
        Key {
            kind,
            name: name.into(),
        }
    }

    #[test]
    fn separates_casks_from_same_named_formulae_and_follows_runtime_receipts() {
        let inventory = parse_inventory(br#"{"formulae":[{"name":"runtime","installed":[{"version":"1"}]},{"name":"client","dependencies":["wrong-new-dependency"],"installed":[{"version":"1","runtime_dependencies":[{"full_name":"runtime","declared_directly":true}]}]}],"casks":[{"token":"runtime","installed":"1"},{"token":"app","installed":"1","depends_on":{"cask":["runtime"],"formula":["client"]}}]}"#).unwrap();
        let graph = InstalledGraph::build(&inventory);
        assert_eq!(
            graph.direct_users(&key(Kind::Cask, "runtime")),
            [key(Kind::Cask, "app")]
        );
        assert_eq!(
            graph.direct_users(&key(Kind::Formula, "runtime")),
            [key(Kind::Formula, "client")]
        );
        assert_eq!(
            graph.indirect_users(&key(Kind::Formula, "runtime")),
            [key(Kind::Cask, "app")]
        );
        assert!(graph.unresolved(&key(Kind::Formula, "client")).is_empty());
    }

    #[test]
    fn ambiguous_taps_are_not_silently_linked() {
        let inventory = parse_inventory(br#"{"formulae":[{"name":"runtime","full_name":"one/tap/runtime","installed":[{"version":"1"}]},{"name":"runtime","full_name":"two/tap/runtime","installed":[{"version":"1"}]}],"casks":[{"token":"app","installed":"1","depends_on":{"formula":["runtime"]}}]}"#).unwrap();
        let graph = InstalledGraph::build(&inventory);
        assert!(graph.needs(&key(Kind::Cask, "app")).is_empty());
        assert_eq!(
            graph.unresolved(&key(Kind::Cask, "app")),
            [key(Kind::Formula, "runtime")]
        );
    }

    #[test]
    fn cycles_terminate_and_never_mark_a_package_as_its_own_consumer() {
        let inventory = parse_inventory(br#"{"formulae":[{"name":"one","installed":[{"version":"1"}],"dependencies":["two"]},{"name":"two","installed":[{"version":"1"}],"dependencies":["one"]}],"casks":[]}"#).unwrap();
        let graph = InstalledGraph::build(&inventory);
        assert!(graph.indirect_users(&key(Kind::Formula, "one")).is_empty());
    }

    #[test]
    fn recursive_and_legacy_receipt_entries_are_never_presented_as_direct() {
        let inventory = parse_inventory(br#"{"formulae":[{"name":"root","dependencies":["not-installed-version"],"installed":[{"version":"1","runtime_dependencies":[{"full_name":"direct","declared_directly":true},{"full_name":"transitive","declared_directly":false},{"full_name":"legacy"}]}]},{"name":"direct","dependencies":["transitive"],"installed":[{"version":"1"}]},{"name":"transitive","installed":[{"version":"1"}]},{"name":"legacy","installed":[{"version":"1"}]}],"casks":[]}"#).unwrap();
        let graph = InstalledGraph::build(&inventory);
        assert_eq!(
            graph.needs(&key(Kind::Formula, "root")),
            [key(Kind::Formula, "direct")]
        );
        assert_eq!(
            graph.direct_users(&key(Kind::Formula, "transitive")),
            [key(Kind::Formula, "direct")]
        );
        assert_eq!(
            graph.runtime_users(&key(Kind::Formula, "transitive")),
            [key(Kind::Formula, "root")]
        );
        assert!(graph.direct_users(&key(Kind::Formula, "legacy")).is_empty());
        assert_eq!(
            graph.runtime_users(&key(Kind::Formula, "legacy")),
            [key(Kind::Formula, "root")]
        );
        assert!(graph.unresolved(&key(Kind::Formula, "root")).is_empty());
    }
}
