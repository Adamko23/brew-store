use crate::assets::{self, Visual};
use crate::brew::{Action, Brew, Details, SharedProgress, Snapshot};
use crate::catalog::{self, Catalog};
use crate::model::{Key, Outdated, Package, merge};
use crate::tools;
use iced::{Subscription, Task, keyboard, window};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PAGE_SIZE: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Search,
    Installed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Apps,
    Tools,
}

impl Scope {
    pub const ALL: [Self; 3] = [Self::Apps, Self::Tools, Self::All];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => crate::i18n::t("Všetko"),
            Self::Apps => crate::i18n::t("Aplikácie"),
            Self::Tools => crate::i18n::t("Nástroje"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    Search(String),
    SubmitSearch,
    DockIcon,
    DragWindow,
    SearchDone {
        generation: u64,
        result: Result<Vec<usize>, String>,
    },
    ThemeChanged(iced::theme::Mode),
    SystemVersion(Option<String>),
    Scope(Scope),
    Select(Key),
    Paginate(usize),
    Cached(Result<Option<Catalog>, String>),
    Catalog(Result<(Catalog, Option<String>), String>),
    Inventory(Result<Snapshot, String>),
    Visual {
        key: Key,
        version: String,
        visual: Visual,
    },
    Detail {
        key: Key,
        result: Result<Box<Details>, String>,
    },
    CloseDetail,
    Refresh,
    Propose(Action),
    Confirm,
    Cancel,
    Finished(Result<usize, String>),
    Tick,
    FocusSearch,
    FocusNext(bool),
    WindowFocused,
    Quit,
    Close(window::Id),
    DismissNotice,
    DismissOperation,
    ToggleLog,
    RetryOperation,
    CheckTools,
    ToolsChecked {
        generation: u64,
        report: tools::Report,
    },
    ProposeTools,
    ToolsFinished {
        generation: u64,
        result: Result<tools::Completion, String>,
    },
    DismissTools,
    ToolsSettings,
    OpenHomepage(String),
    OpenApplication(Key),
    OpenSelected,
    HomepageOpened(Result<(), String>),
}

pub struct Operation {
    pub action: Action,
    pub target_version: Option<String>,
    pub effect: Effect,
    pub title: String,
    pub progress: SharedProgress,
    pub result: Option<Result<usize, String>>,
    pub started: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Unchecked,
    Applied,
    NotApplied,
}

impl Operation {
    fn reconcile(&mut self, inventory: &[Package], outdated: &Result<Vec<Outdated>, String>) {
        if !matches!(self.result, Some(Err(_))) {
            return;
        }
        if matches!(self.action, Action::UpgradeAll) {
            self.effect = match outdated {
                Ok(updates) if updates.iter().all(|update| update.pinned) => Effect::Applied,
                Ok(_) => Effect::NotApplied,
                Err(_) => Effect::Unchecked,
            };
            return;
        }
        let Some(key) = self.action.key() else {
            return;
        };
        let package = inventory
            .iter()
            .find(|p| &p.key == key && p.installed.is_some());
        self.effect = match &self.action {
            Action::Install(_) | Action::Upgrade(_) => {
                let Some(target) = self
                    .target_version
                    .as_deref()
                    .filter(|v| !v.is_empty() && *v != "latest" && *v != "HEAD")
                else {
                    return;
                };
                if package
                    .and_then(|p| p.installed.as_ref())
                    .is_some_and(|i| i.versions.iter().any(|v| v == target))
                {
                    Effect::Applied
                } else {
                    Effect::NotApplied
                }
            }
            Action::UpgradeAll => unreachable!(),
            Action::Uninstall(_) | Action::RemoveStale(_) => {
                if package.is_none() {
                    Effect::Applied
                } else {
                    Effect::NotApplied
                }
            }
        };
    }
}

#[derive(Debug, Clone)]
pub enum Confirmation {
    Package(Action),
    DeveloperTools(tools::Plan),
}

pub struct ToolsRepair {
    pub checking: bool,
    pub running: bool,
    pub report: Option<tools::Report>,
    pub progress: SharedProgress,
    pub result: Option<Result<String, String>>,
}

pub struct App {
    pub directory: PathBuf,
    pub brew: Result<Brew, String>,
    pub catalog: Vec<Package>,
    pub inventory: Vec<Package>,
    pub graph: crate::dependencies::InstalledGraph,
    updates: Vec<Outdated>,
    pub packages: Arc<Vec<Package>>,
    pub theme_mode: iced::theme::Mode,
    pub macos_version: Option<String>,
    forced_theme: bool,
    pub search_loading: bool,
    search_generation: u64,
    cellar: Option<PathBuf>,
    pub matches: Vec<usize>,
    search_task: Option<iced::task::Handle>,
    reset_offset_after_search: bool,
    dock_icon_set: bool,
    pub page: Page,
    pub scope: Scope,
    pub search: String,
    pub installed_search: String,
    pub selected: Option<Key>,
    pub detail_package: Option<Package>,
    pub detail_from_installation: bool,
    pub detail_loading: bool,
    pub detail_error: Option<String>,
    pub offset: usize,
    pub catalog_loading: bool,
    pub inventory_loading: bool,
    pub inventory_fresh: bool,
    pub updates_fresh: bool,
    pub catalog_error: Option<String>,
    pub inventory_error: Option<String>,
    pub updates_error: Option<String>,
    pub catalog_time: Option<u64>,
    pub confirmation: Option<Confirmation>,
    confirmation_accept_focused: bool,
    pub operation: Option<Operation>,
    pub show_log: bool,
    pub tools_repair: Option<ToolsRepair>,
    tools_generation: u64,
    tools_focus: usize,
    tools_task: Option<iced::task::Handle>,
    pub notice: Option<String>,
    pub visuals: BTreeMap<Key, Visual>,
    visual_versions: BTreeMap<Key, String>,
    pending_visuals: BTreeSet<(Key, String)>,
    asset_gate: Arc<tokio::sync::Semaphore>,
    last_inventory_checked: Option<Instant>,
}

impl App {
    pub fn boot(directory: PathBuf) -> (Self, Task<Message>) {
        Self::boot_with_theme(directory, None)
    }

    pub fn boot_with_theme(
        directory: PathBuf,
        forced: Option<iced::theme::Mode>,
    ) -> (Self, Task<Message>) {
        let mut app = Self {
            directory,
            brew: Brew::discover(),
            catalog: vec![],
            inventory: vec![],
            graph: crate::dependencies::InstalledGraph::default(),
            updates: vec![],
            packages: Arc::new(vec![]),
            theme_mode: forced.unwrap_or(iced::theme::Mode::None),
            macos_version: None,
            forced_theme: forced.is_some(),
            search_loading: false,
            search_generation: 0,
            cellar: None,
            matches: vec![],
            search_task: None,
            reset_offset_after_search: false,
            dock_icon_set: false,
            page: Page::Search,
            scope: Scope::All,
            search: String::new(),
            installed_search: String::new(),
            selected: None,
            detail_package: None,
            detail_from_installation: false,
            detail_loading: false,
            detail_error: None,
            offset: 0,
            catalog_loading: true,
            inventory_loading: false,
            inventory_fresh: false,
            updates_fresh: false,
            catalog_error: None,
            inventory_error: None,
            updates_error: None,
            catalog_time: None,
            confirmation: None,
            confirmation_accept_focused: false,
            operation: None,
            show_log: false,
            tools_repair: None,
            tools_generation: 0,
            tools_focus: 0,
            tools_task: None,
            notice: None,
            visuals: BTreeMap::new(),
            visual_versions: BTreeMap::new(),
            pending_visuals: BTreeSet::new(),
            asset_gate: Arc::new(tokio::sync::Semaphore::new(2)),
            last_inventory_checked: None,
        };
        let directory = app.directory.clone();
        let cache = Task::perform(
            async move { catalog::read_cache(&directory).await },
            Message::Cached,
        );
        let inventory = app.load_inventory();
        (
            app,
            Task::batch([
                cache,
                inventory,
                iced::system::theme().map(Message::ThemeChanged),
                Task::perform(tools::system_version(), Message::SystemVersion),
            ]),
        )
    }

    pub fn query(&self) -> &str {
        match self.page {
            Page::Search => &self.search,
            Page::Installed => &self.installed_search,
        }
    }

    pub fn busy(&self) -> bool {
        self.operation.as_ref().is_some_and(|o| o.result.is_none())
            || self
                .tools_repair
                .as_ref()
                .is_some_and(|repair| repair.running)
    }

    pub fn retry_action(&self) -> Option<Action> {
        let operation = self.operation.as_ref()?;
        (matches!(operation.result, Some(Err(_)))
            && operation.effect != Effect::Applied
            && self.valid_action(&operation.action))
        .then(|| operation.action.clone())
    }

    pub fn can_upgrade_all(&self) -> bool {
        self.can_change() && self.updates_fresh && self.updates.iter().any(|update| !update.pinned)
    }

    pub fn can_change(&self) -> bool {
        self.brew.is_ok() && self.inventory_fresh && !self.detail_loading && !self.busy()
    }

    pub fn required_macos<'a>(&self, package: &'a Package) -> Option<&'a str> {
        let minimum = package.minimum_macos.as_deref()?;
        let current = self.macos_version.as_deref()?;
        (!tools::at_least(current, minimum)).then_some(minimum)
    }

    pub fn selected_package(&self) -> Option<&Package> {
        self.detail_package.as_ref().or_else(|| {
            self.selected
                .as_ref()
                .and_then(|key| self.packages.iter().find(|p| &p.key == key))
        })
    }

    fn load_inventory(&mut self) -> Task<Message> {
        if let Ok(brew) = &self.brew {
            self.inventory_loading = true;
            let brew = brew.clone();
            Task::perform(async move { brew.snapshot().await }, Message::Inventory)
        } else {
            Task::none()
        }
    }

    fn load_catalog(&mut self) -> Task<Message> {
        self.catalog_loading = true;
        Task::perform(catalog::refresh(self.directory.clone()), Message::Catalog)
    }

    fn rebuild(&mut self) -> Task<Message> {
        self.packages = Arc::new(merge(&self.catalog, &self.inventory, &self.updates));
        self.matches.clear();
        self.detail_package = None;
        if self
            .selected
            .as_ref()
            .is_some_and(|key| !self.packages.iter().any(|p| &p.key == key))
        {
            self.selected = None;
        }
        self.filter()
    }

    fn filter(&mut self) -> Task<Message> {
        self.filter_after(Duration::ZERO)
    }

    fn filter_after(&mut self, delay: Duration) -> Task<Message> {
        if let Some(task) = self.search_task.take() {
            task.abort();
        }
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        let query = self.query().trim().to_owned();
        if self.page == Page::Search && query.is_empty() {
            self.matches.clear();
            self.offset = 0;
            self.search_loading = false;
            return Task::none();
        }
        // Keep existing rows while the next query is pending. Never blank the
        // list per keystroke, and never do matching or sorting on the UI thread.
        self.search_loading = true;
        let packages = self.packages.clone();
        let (page, scope) = (self.page, self.scope);
        let task = Task::perform(
            async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                let result = tokio::task::spawn_blocking(move || {
                    crate::search::find(&packages, page, scope, &query)
                })
                .await
                .map_err(|e| e.to_string());
                Message::SearchDone { generation, result }
            },
            |message| message,
        );
        let (task, handle) = task.abortable();
        self.search_task = Some(handle.abort_on_drop());
        task
    }

    fn load_visible_visuals(&mut self) -> Task<Message> {
        let Some(cellar) = &self.cellar else {
            return Task::none();
        };
        let keys: Vec<_> = if let Some(key) = &self.selected {
            vec![key.clone()]
        } else {
            self.matches
                .iter()
                .skip(self.offset)
                .take(PAGE_SIZE)
                .map(|&index| self.packages[index].key.clone())
                .collect()
        };
        let mut tasks = Vec::new();
        for key in keys {
            let Some(package) = self.inventory.iter().find(|p| p.key == key) else {
                continue;
            };
            let key = package.key.clone();
            let version = package.installed_version();
            if self.visual_versions.get(&key) == Some(&version) {
                continue;
            }
            if !self.pending_visuals.insert((key.clone(), version.clone())) {
                continue;
            }
            self.visuals.remove(&key);
            let future = assets::inspect(
                package.clone(),
                cellar.clone(),
                self.directory.clone(),
                self.asset_gate.clone(),
            );
            tasks.push(Task::perform(future, move |visual| Message::Visual {
                key: key.clone(),
                version: version.clone(),
                visual,
            }));
        }
        Task::batch(tasks)
    }

    fn valid_action(&self, action: &Action) -> bool {
        if !self.can_change() {
            return false;
        }
        if matches!(action, Action::UpgradeAll) {
            return self.can_upgrade_all();
        }
        let Some(key) = action.key() else {
            return false;
        };
        let Some(package) = self.packages.iter().find(|p| &p.key == key) else {
            return false;
        };
        match action {
            Action::UpgradeAll => false,
            Action::Install(_) => {
                package.installed.is_none()
                    && !package.disabled
                    && self.required_macos(package).is_none()
            }
            Action::Upgrade(_) => {
                package.installed.is_some()
                    && package.outdated
                    && self.updates_fresh
                    && !package.is_pinned()
                    && !package.disabled
                    && self.required_macos(package).is_none()
            }
            Action::Uninstall(_) => package.installed.is_some(),
            Action::RemoveStale(_) => {
                package.installed.is_some()
                    && package.key.kind == crate::model::Kind::Cask
                    && self.visuals.get(key).is_some_and(|v| v.app_missing)
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if self.tools_repair.is_some()
            && self.confirmation.is_none()
            && matches!(
                &message,
                Message::Navigate(_)
                    | Message::Search(_)
                    | Message::SubmitSearch
                    | Message::Scope(_)
                    | Message::Select(_)
                    | Message::Paginate(_)
                    | Message::Propose(_)
                    | Message::OpenApplication(_)
                    | Message::OpenSelected
                    | Message::FocusSearch
                    | Message::Refresh
            )
        {
            return Task::none();
        }

        if self.confirmation.is_some()
            && !matches!(
                &message,
                Message::Confirm
                    | Message::Cancel
                    | Message::FocusNext(_)
                    | Message::Quit
                    | Message::Close(_)
                    | Message::Cached(_)
                    | Message::Catalog(_)
                    | Message::Inventory(_)
                    | Message::Finished(_)
                    | Message::Tick
                    | Message::HomepageOpened(_)
                    | Message::ToolsChecked { .. }
                    | Message::ToolsFinished { .. }
                    | Message::Visual { .. }
                    | Message::Detail { .. }
                    | Message::SearchDone { .. }
                    | Message::ThemeChanged(_)
                    | Message::SystemVersion(_)
                    | Message::DockIcon
            )
        {
            return Task::none();
        }
        match message {
            Message::Navigate(page) => {
                self.matches.clear();
                self.page = page;
                self.scope = if page == Page::Installed {
                    Scope::Apps
                } else {
                    Scope::All
                };
                self.offset = 0;
                self.selected = None;
                self.detail_package = None;
                self.detail_loading = false;
                return self.filter();
            }
            Message::Search(query) => {
                if self.query() == query {
                    return Task::none();
                }
                match self.page {
                    Page::Search => self.search = query,
                    Page::Installed => self.installed_search = query,
                }
                self.reset_offset_after_search = true;
                return self.filter_after(Duration::from_millis(180));
            }
            Message::DockIcon => {
                if !self.dock_icon_set {
                    match crate::native::set_dock_icon() {
                        Ok(()) => self.dock_icon_set = true,
                        Err(error) => self.notice = Some(error),
                    }
                }
            }
            Message::DragWindow => return window::latest().and_then(window::drag),
            Message::SubmitSearch => {
                // Enter only flushes a pending debounce, without navigating.
                if !self.search_loading {
                    return Task::none();
                }
                return self.filter();
            }
            Message::SearchDone { generation, result } => {
                if generation == self.search_generation {
                    self.search_task = None;
                    self.search_loading = false;
                    if self.reset_offset_after_search {
                        self.offset = 0;
                        self.reset_offset_after_search = false;
                    }
                    match result {
                        Ok(matches) => self.matches = matches,
                        Err(error) => self.notice = Some(error),
                    }
                    if self.offset >= self.matches.len() {
                        self.offset = 0;
                    }
                    return self.load_visible_visuals();
                }
            }
            Message::ThemeChanged(mode) => {
                if !self.forced_theme {
                    self.theme_mode = mode;
                }
            }
            Message::SystemVersion(version) => self.macos_version = version,
            Message::Scope(scope) => {
                self.scope = scope;
                self.selected = None;
                self.detail_package = None;
                self.detail_loading = false;
                self.offset = 0;
                return self.filter();
            }
            Message::Select(key) => {
                self.selected = Some(key.clone());
                self.detail_package = None;
                self.detail_from_installation = false;
                self.detail_error = None;
                let visual = self.load_visible_visuals();
                if let Ok(brew) = &self.brew
                    && !self.busy()
                {
                    self.detail_loading = true;
                    let brew = brew.clone();
                    let detail = Task::perform(
                        async move {
                            let result = brew.details(&key).await.map(Box::new);
                            Message::Detail { key, result }
                        },
                        |message| message,
                    );
                    return Task::batch([detail, visual]);
                }
                return visual;
            }
            Message::Detail { key, result } => {
                if self.selected.as_ref() == Some(&key) {
                    self.detail_loading = false;
                    match result {
                        Ok(details) => {
                            self.detail_from_installation = details.from_installation;
                            let mut package = details.package;
                            if let Some(current) = self.packages.iter().find(|p| p.key == key) {
                                package.installed = current.installed.clone();
                                package.outdated = current.outdated;
                                package.version = current.version.clone();
                            }
                            self.detail_package = Some(package);
                        }
                        Err(error) => self.detail_error = Some(error),
                    }
                }
            }
            Message::CloseDetail => {
                self.selected = None;
                self.detail_package = None;
                self.detail_loading = false;
            }
            Message::Paginate(offset) => {
                self.offset = offset;
                return self.load_visible_visuals();
            }
            Message::Cached(result) => {
                match result {
                    Ok(Some(catalog)) => {
                        self.catalog_time = Some(catalog.fetched_at);
                        self.catalog = catalog.packages;
                        let search = self.rebuild();
                        let refresh = self.load_catalog();
                        return Task::batch([search, refresh]);
                    }
                    Ok(None) => (),
                    Err(error) => self.catalog_error = Some(error),
                }
                return self.load_catalog();
            }
            Message::Catalog(result) => {
                self.catalog_loading = false;
                match result {
                    Ok((catalog, warning)) => {
                        self.catalog_time = Some(catalog.fetched_at);
                        self.catalog = catalog.packages;
                        self.catalog_error = warning;
                        return self.rebuild();
                    }
                    Err(error) => self.catalog_error = Some(error),
                }
            }
            Message::Inventory(result) => {
                self.inventory_loading = false;
                match result {
                    Ok(snapshot) => {
                        if let Some(operation) = &mut self.operation {
                            operation.reconcile(&snapshot.packages, &snapshot.outdated);
                        }
                        let changed = self.inventory != snapshot.packages
                            || snapshot
                                .outdated
                                .as_ref()
                                .is_ok_and(|updates| updates != &self.updates);
                        self.last_inventory_checked = Some(Instant::now());
                        self.inventory = snapshot.packages;
                        if changed {
                            self.graph =
                                crate::dependencies::InstalledGraph::build(&self.inventory);
                        }
                        self.inventory_fresh = true;
                        self.inventory_error = None;
                        match snapshot.outdated {
                            Ok(updates) => {
                                self.updates = updates;
                                self.updates_fresh = true;
                                self.updates_error = None;
                            }
                            Err(error) => {
                                self.updates_fresh = false;
                                self.updates_error = Some(error);
                            }
                        }
                        self.visuals
                            .retain(|key, _| self.inventory.iter().any(|p| &p.key == key));
                        self.visual_versions
                            .retain(|key, _| self.inventory.iter().any(|p| &p.key == key));
                        self.cellar = Some(snapshot.cellar);
                        if !changed {
                            return self.load_visible_visuals();
                        }
                        return self.rebuild();
                    }
                    Err(error) => {
                        self.inventory_fresh = false;
                        self.updates_fresh = false;
                        self.inventory_error = Some(error);
                    }
                }
            }
            Message::Visual {
                key,
                version,
                visual,
            } => {
                self.pending_visuals.remove(&(key.clone(), version.clone()));
                if self
                    .inventory
                    .iter()
                    .any(|p| p.key == key && p.installed_version() == version)
                {
                    self.visual_versions.insert(key.clone(), version);
                    self.visuals.insert(key, visual);
                }
            }
            Message::Refresh if !self.busy() => {
                if self.brew.is_err() {
                    self.brew = Brew::discover();
                }
                self.visual_versions.clear();
                let inventory = if self.inventory_loading {
                    Task::none()
                } else {
                    self.load_inventory()
                };
                let catalog = if self.catalog_loading {
                    Task::none()
                } else {
                    self.load_catalog()
                };
                return Task::batch([inventory, catalog]);
            }
            Message::Propose(action) => {
                if self.valid_action(&action) {
                    self.confirmation = Some(Confirmation::Package(action));
                    self.confirmation_accept_focused = false;
                    return iced::widget::operation::focus("confirmation-cancel");
                }
            }
            Message::Confirm => {
                let Some(confirmation) = self.confirmation.take() else {
                    return Task::none();
                };
                match confirmation {
                    Confirmation::Package(action) => {
                        if self.inventory_loading {
                            self.confirmation = Some(Confirmation::Package(action));
                            return Task::none();
                        }
                        if !self.valid_action(&action) {
                            self.notice = Some(
                                crate::i18n::t(
                                    "Stav Homebrew sa zmenil. Obnov údaje a vyber operáciu znovu.",
                                )
                                .into(),
                            );
                            return Task::none();
                        }
                        let Ok(brew) = &self.brew else {
                            return Task::none();
                        };
                        let brew = brew.clone();
                        let directory = self.directory.clone();
                        let progress = SharedProgress::default();
                        self.tools_task = None;
                        self.tools_generation = self.tools_generation.wrapping_add(1);
                        self.tools_repair = None;
                        let target_version = self
                            .packages
                            .iter()
                            .find(|p| Some(&p.key) == action.key())
                            .map(|p| p.version.clone());
                        self.operation = Some(Operation {
                            title: action.label(),
                            action: action.clone(),
                            target_version,
                            effect: Effect::Unchecked,
                            progress: progress.clone(),
                            result: None,
                            started: Instant::now(),
                        });
                        self.show_log = false;
                        return Task::perform(
                            async move { brew.run(vec![action], directory, progress).await },
                            Message::Finished,
                        );
                    }
                    Confirmation::DeveloperTools(plan) => {
                        if self.busy() {
                            return Task::none();
                        }
                        let Some(repair) = &mut self.tools_repair else {
                            return Task::none();
                        };
                        if repair.checking
                            || repair
                                .report
                                .as_ref()
                                .is_none_or(|report| report.plan() != plan)
                        {
                            return Task::none();
                        }
                        repair.running = true;
                        repair.result = None;
                        repair.progress = SharedProgress::default();
                        let progress = repair.progress.clone();
                        let generation = self.tools_generation;
                        let directory = self.directory.clone();
                        return Task::perform(
                            async move {
                                let result = tools::run(plan, directory, progress).await;
                                Message::ToolsFinished { generation, result }
                            },
                            |message| message,
                        );
                    }
                }
            }
            Message::Cancel => {
                if self.confirmation.take().is_some() {
                    return iced::widget::operation::focus(if self.tools_repair.is_some() {
                        "tools-primary"
                    } else {
                        "catalog-search"
                    });
                }
                if self.tools_repair.is_some() && !self.busy() {
                    return self.update(Message::DismissTools);
                }
                if self.selected.is_some() {
                    return self.update(Message::CloseDetail);
                }
            }
            Message::Finished(result) => {
                if let Some(operation) = &mut self.operation {
                    operation.result = Some(result);
                }
                self.visual_versions.clear();
                self.inventory_fresh = false;
                return self.load_inventory();
            }
            Message::RetryOperation => {
                if let Some(action) = self.retry_action() {
                    return self.update(Message::Propose(action));
                }
            }
            Message::CheckTools => {
                if self.busy() || self.tools_repair.as_ref().is_some_and(|r| r.checking) {
                    return Task::none();
                }
                let issue = self
                    .operation
                    .as_ref()
                    .and_then(|operation| {
                        crate::brew::progress_snapshot(&operation.progress).developer_tools
                    })
                    .unwrap_or_default();
                self.tools_generation = self.tools_generation.wrapping_add(1);
                self.tools_focus = 0;
                let generation = self.tools_generation;
                self.tools_repair = Some(ToolsRepair {
                    checking: true,
                    running: false,
                    report: None,
                    progress: SharedProgress::default(),
                    result: None,
                });
                let (task, handle) = Task::perform(
                    async move {
                        Message::ToolsChecked {
                            generation,
                            report: tools::check(issue).await,
                        }
                    },
                    |message| message,
                )
                .abortable();
                self.tools_task = Some(handle.abort_on_drop());
                return Task::batch([task, iced::widget::operation::focus("tools-close")]);
            }
            Message::ToolsChecked { generation, report } => {
                if generation == self.tools_generation
                    && let Some(repair) = &mut self.tools_repair
                {
                    repair.checking = false;
                    repair.report = Some(report);
                    self.tools_task = None;
                }
            }
            Message::ProposeTools => {
                if self.busy() {
                    return Task::none();
                }
                if let Some(report) = self
                    .tools_repair
                    .as_ref()
                    .filter(|r| !r.checking)
                    .and_then(|r| r.report.as_ref())
                {
                    self.confirmation = Some(Confirmation::DeveloperTools(report.plan()));
                    self.confirmation_accept_focused = false;
                    return iced::widget::operation::focus("confirmation-cancel");
                }
            }
            Message::ToolsFinished { generation, result } => {
                if generation == self.tools_generation
                    && let Some(repair) = &mut self.tools_repair
                {
                    repair.running = false;
                    repair.result = Some(result.map(|completion| {
                        if let Some(version) = completion.installed_version
                            && let Some(report) = &mut repair.report
                        {
                            report.installed_version = Some(version);
                            report.files_present = true;
                            report.update = None;
                        }
                        completion.message
                    }));
                    return self.load_inventory();
                }
            }
            Message::DismissTools => {
                if !self.busy() {
                    self.tools_task = None;
                    self.tools_generation = self.tools_generation.wrapping_add(1);
                    self.tools_repair = None;
                }
            }
            Message::ToolsSettings => {
                return Task::perform(tools::open_settings(), Message::HomepageOpened);
            }
            Message::Tick => (),
            Message::DismissOperation => {
                if !self.busy() {
                    self.operation = None;
                }
            }
            Message::ToggleLog => self.show_log = !self.show_log,
            Message::FocusSearch => {
                if self.page != Page::Search {
                    self.matches.clear();
                }
                self.page = Page::Search;
                self.scope = Scope::All;
                self.offset = 0;
                self.selected = None;
                self.detail_loading = false;
                let search = self.filter();
                return Task::batch([search, iced::widget::operation::focus("catalog-search")]);
            }
            Message::FocusNext(previous) => {
                if self.confirmation.is_some() {
                    self.confirmation_accept_focused = !self.confirmation_accept_focused;
                    return iced::widget::operation::focus(if self.confirmation_accept_focused {
                        "confirmation-accept"
                    } else {
                        "confirmation-cancel"
                    });
                }
                if let Some(repair) = &self.tools_repair {
                    let ids: &[&str] = if repair.checking {
                        &["tools-close"]
                    } else if repair.running {
                        &["tools-settings", "tools-download"]
                    } else {
                        &[
                            "tools-close",
                            "tools-primary",
                            "tools-recheck",
                            "tools-settings",
                            "tools-download",
                        ]
                    };
                    self.tools_focus = if previous {
                        (self.tools_focus + ids.len() - 1) % ids.len()
                    } else {
                        (self.tools_focus + 1) % ids.len()
                    };
                    return iced::widget::operation::focus(ids[self.tools_focus]);
                }
                return if previous {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                };
            }
            Message::WindowFocused => {
                if !self.busy()
                    && !self.inventory_loading
                    && self
                        .last_inventory_checked
                        .is_none_or(|t| t.elapsed() > Duration::from_secs(2))
                {
                    return self.load_inventory();
                }
            }
            Message::Quit | Message::Close(_) if self.busy() => {
                self.notice = Some(
                    crate::i18n::t(
                        "Homebrew práve pracuje. Aplikáciu môžeš zavrieť po dokončení operácie.",
                    )
                    .into(),
                )
            }
            Message::Quit => return iced::exit(),
            Message::Close(id) => return window::close(id),
            Message::DismissNotice => self.notice = None,
            Message::OpenSelected => {
                let key = self.selected.clone().or_else(|| {
                    if self.matches.len() == 1 {
                        Some(self.packages[self.matches[0]].key.clone())
                    } else {
                        None
                    }
                });
                if let Some(key) = key {
                    return self.update(Message::OpenApplication(key));
                }
            }
            Message::OpenApplication(key) => {
                if self.busy() {
                    return Task::none();
                }
                let Some(package) = self.inventory.iter().find(|package| package.key == key) else {
                    return Task::none();
                };
                let Some(bundle) = self
                    .visuals
                    .get(&key)
                    .and_then(|visual| visual.app_bundle.clone())
                else {
                    return Task::none();
                };
                let package = package.clone();
                return Task::perform(
                    async move { assets::open_application(&package, &bundle).await },
                    Message::HomepageOpened,
                );
            }
            Message::OpenHomepage(url) => {
                return Task::perform(open_homepage(url), Message::HomepageOpened);
            }
            Message::HomepageOpened(Err(error)) => self.notice = Some(error),
            Message::HomepageOpened(Ok(())) | Message::Refresh => (),
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = iced::event::listen_with(|event, _, _| {
            if matches!(event, iced::Event::Window(window::Event::Focused)) {
                return Some(Message::WindowFocused);
            }
            let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            else {
                return None;
            };
            match key {
                keyboard::Key::Character(value) if modifiers.command() => match value.as_str() {
                    "f" | "F" => Some(Message::FocusSearch),
                    "r" | "R" => Some(Message::Refresh),
                    "o" | "O" => Some(Message::OpenSelected),
                    "," => Some(Message::CheckTools),
                    "u" | "U" if modifiers.shift() => Some(Message::Propose(Action::UpgradeAll)),
                    "q" | "Q" | "w" | "W" => Some(Message::Quit),
                    "1" => Some(Message::Navigate(Page::Search)),
                    "2" => Some(Message::Navigate(Page::Installed)),
                    _ => None,
                },
                keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Cancel),
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    Some(Message::FocusNext(modifiers.shift()))
                }
                _ => None,
            }
        });
        let timer = if self.busy() {
            iced::time::every(Duration::from_millis(200)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            keys,
            timer,
            window::close_requests().map(Message::Close),
            iced::system::theme_changes().map(Message::ThemeChanged),
            window::open_events().map(|_| Message::DockIcon),
        ])
    }
}

async fn open_homepage(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url)
        .map_err(|_| crate::i18n::t("Neplatná webová adresa.").to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(crate::i18n::t("Podporované sú iba adresy HTTP a HTTPS.").into());
    }
    let status = tokio::process::Command::new("/usr/bin/open")
        .arg(parsed.as_str())
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(crate::i18n::t("Web sa nepodarilo otvoriť.").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, parse_catalog, parse_inventory};

    fn app() -> App {
        let (mut app, _) = App::boot(PathBuf::from("/unused"));
        app.catalog = parse_catalog(br#"[{"token":"editor","name":["Useful Editor"],"desc":"Write source code","version":"2"},{"token":"browser","name":["Useful Browser"],"version":"3"}]"#, Kind::Cask).unwrap();
        let _ = app.rebuild();
        app
    }

    #[test]
    fn live_search_keeps_rows_until_the_latest_background_result_arrives() {
        let mut app = app();
        let _ = app.update(Message::Search("editor".into()));
        let matches = crate::search::find(&app.packages, app.page, app.scope, app.query());
        let _ = app.update(Message::SearchDone {
            generation: app.search_generation,
            result: Ok(matches),
        });
        let old_rows = app.matches.clone();
        let _ = app.update(Message::Search("browser".into()));
        assert!(app.search_loading);
        assert_eq!(app.matches, old_rows);
        let matches = crate::search::find(&app.packages, app.page, app.scope, app.query());
        let _ = app.update(Message::SearchDone {
            generation: app.search_generation,
            result: Ok(matches),
        });
        assert_eq!(app.packages[app.matches[0]].token, "browser");
        assert_eq!(app.page, Page::Search);
    }

    #[test]
    fn clearing_search_invalidates_pending_results_and_enter_never_navigates() {
        let mut app = app();
        let _ = app.update(Message::Search("editor".into()));
        let old_generation = app.search_generation;
        let _ = app.update(Message::Search(String::new()));
        let _ = app.update(Message::SearchDone {
            generation: old_generation,
            result: Ok(vec![0]),
        });
        assert!(app.matches.is_empty());
        assert!(!app.search_loading);
        let _ = app.update(Message::SubmitSearch);
        assert_eq!(app.page, Page::Search);
        assert!(app.selected.is_none());
    }

    #[test]
    fn late_background_results_never_replace_a_newer_search() {
        let mut app = app();
        let _ = app.update(Message::Search("editor".into()));
        let _ = app.update(Message::SubmitSearch);
        let old = app.search_generation;
        let _ = app.update(Message::Search("browser".into()));
        let _ = app.update(Message::SubmitSearch);
        let _ = app.update(Message::SearchDone {
            generation: old,
            result: Ok(vec![0]),
        });
        assert!(app.matches.is_empty());
        assert!(app.search_loading);
    }
    #[test]
    fn mutations_require_a_known_inventory_and_an_explicit_confirmation() {
        let mut app = app();
        let key = app.packages[0].key.clone();
        let _ = app.update(Message::Propose(Action::Install(key)));
        assert!(app.confirmation.is_none());
        let _ = app.update(Message::Confirm);
        assert!(!app.busy());
    }
    #[test]
    fn installed_only_detail_is_explained_without_blocking_removal_or_leaking_to_another_selection()
    {
        let mut app = app();
        app.inventory = parse_inventory(
            br#"{"formulae":[{"name":"evcc","full_name":"evcc-io/tap/evcc","installed":[{"version":"0.132.1"}]}],"casks":[]}"#,
        ).unwrap();
        app.inventory_fresh = true;
        let _ = app.rebuild();
        let package = app.inventory[0].clone();
        let key = package.key.clone();
        let _ = app.update(Message::Select(key.clone()));
        let _ = app.update(Message::Detail {
            key: key.clone(),
            result: Ok(Box::new(Details {
                package,
                from_installation: true,
            })),
        });
        assert!(app.detail_from_installation);
        assert!(app.detail_error.is_none());
        assert!(app.valid_action(&Action::Uninstall(key)));
        let other = app.catalog[0].key.clone();
        let _ = app.update(Message::Select(other));
        assert!(!app.detail_from_installation);
    }
    #[test]
    fn installed_page_uses_actual_inventory_not_catalog_installation_flags() {
        let mut app = app();
        app.inventory = parse_inventory(
            br#"{"formulae":[],"casks":[{"token":"editor","installed":"1","version":"2"}]}"#,
        )
        .unwrap();
        let _ = app.rebuild();
        let _ = app.update(Message::Navigate(Page::Installed));
        let matches = crate::search::find(&app.packages, app.page, app.scope, app.query());
        let _ = app.update(Message::SearchDone {
            generation: app.search_generation,
            result: Ok(matches),
        });
        assert_eq!(app.matches.len(), 1);
        assert_eq!(app.packages[app.matches[0]].installed_version(), "1");
    }
    #[test]
    fn modal_blocks_background_navigation_and_escape_never_executes() {
        let mut app = app();
        app.confirmation = Some(Confirmation::Package(Action::Install(
            app.packages[0].key.clone(),
        )));
        let _ = app.update(Message::Navigate(Page::Installed));
        assert_eq!(app.page, Page::Search);
        let _ = app.update(Message::Cancel);
        assert!(app.confirmation.is_none());
        assert!(!app.busy());
    }
    #[test]
    fn failed_refresh_keeps_inventory_visible_and_disables_changes() {
        let mut app = app();
        app.inventory = parse_inventory(
            br#"{"formulae":[],"casks":[{"token":"editor","installed":"1","version":"2"}]}"#,
        )
        .unwrap();
        app.inventory_fresh = true;
        let _ = app.rebuild();
        let _ = app.update(Message::Inventory(Err("unavailable".into())));
        assert!(!app.can_change());
        assert_eq!(app.inventory.len(), 1);
    }
    #[test]
    fn returning_to_window_refreshes_inventory_after_debounce() {
        let mut app = app();
        app.inventory_loading = false;
        app.last_inventory_checked = Instant::now().checked_sub(Duration::from_secs(3));
        let _ = app.update(Message::WindowFocused);
        assert!(app.inventory_loading);
    }

    fn failed_upgrade(app: &App) -> Operation {
        Operation {
            action: Action::Upgrade(
                app.packages
                    .iter()
                    .find(|p| p.token == "editor")
                    .unwrap()
                    .key
                    .clone(),
            ),
            title: "Aktualizovať editor".into(),
            target_version: Some("2".into()),
            effect: Effect::Unchecked,
            progress: SharedProgress::default(),
            result: Some(Err("Homebrew failed after install".into())),
            started: Instant::now(),
        }
    }

    #[test]
    fn a_failed_exit_with_the_target_installed_is_partial_completion_not_a_retry() {
        let mut app = app();
        app.operation = Some(failed_upgrade(&app));
        let packages = parse_inventory(
            br#"{"formulae":[],"casks":[{"token":"editor","installed":"2","version":"2"}]}"#,
        )
        .unwrap();
        let _ = app.update(Message::Inventory(Ok(Snapshot {
            packages,
            outdated: Ok(vec![]),
            cellar: "/unused".into(),
        })));
        assert_eq!(app.operation.as_ref().unwrap().effect, Effect::Applied);
        assert!(matches!(
            app.operation.as_ref().unwrap().result,
            Some(Err(_))
        ));
        assert!(app.retry_action().is_none());
    }

    #[test]
    fn a_failed_upgrade_keeps_a_retry_when_the_old_version_is_still_installed() {
        let mut app = app();
        app.operation = Some(failed_upgrade(&app));
        let packages = parse_inventory(
            br#"{"formulae":[],"casks":[{"token":"editor","installed":"1","version":"2"}]}"#,
        )
        .unwrap();
        let outdated = crate::model::parse_outdated(
            br#"{"formulae":[],"casks":[{"name":"editor","current_version":"2"}]}"#,
        )
        .unwrap();
        let _ = app.update(Message::Inventory(Ok(Snapshot {
            packages,
            outdated: Ok(outdated),
            cellar: "/unused".into(),
        })));
        assert_eq!(app.operation.as_ref().unwrap().effect, Effect::NotApplied);
        assert!(app.retry_action().is_some());
        let _ = app.update(Message::RetryOperation);
        assert!(matches!(
            app.confirmation,
            Some(Confirmation::Package(Action::Upgrade(_)))
        ));
        assert!(!app.busy());
    }

    #[test]
    fn a_declared_newer_macos_blocks_installation_and_becomes_allowed_after_system_update() {
        let mut app = app();
        app.inventory_fresh = true;
        app.inventory_loading = false;
        let mut packages = app.packages.as_ref().clone();
        packages[0].minimum_macos = Some("28".into());
        let key = packages[0].key.clone();
        app.packages = Arc::new(packages);
        let _ = app.update(Message::SystemVersion(Some("27.0.1".into())));
        assert_eq!(app.required_macos(&app.packages[0]), Some("28"));
        let _ = app.update(Message::Propose(Action::Install(key.clone())));
        assert!(app.confirmation.is_none());
        let _ = app.update(Message::SystemVersion(Some("28.0".into())));
        let _ = app.update(Message::Propose(Action::Install(key)));
        assert!(app.confirmation.is_some());
    }

    #[test]
    fn an_unprepared_tools_confirmation_cannot_start_a_system_change() {
        let mut app = app();
        app.confirmation = Some(Confirmation::DeveloperTools(tools::Plan::Installer));
        let _ = app.update(Message::Confirm);
        assert!(app.tools_repair.is_none());
        assert!(!app.busy());
    }

    #[test]
    fn bulk_upgrade_requires_fresh_unpinned_updates_and_confirmation() {
        let mut app = app();
        let _ = app.update(Message::Propose(Action::UpgradeAll));
        assert!(app.confirmation.is_none());
        app.inventory_loading = false;
        app.inventory_fresh = true;
        app.updates_fresh = true;
        app.updates = vec![Outdated {
            key: app.packages[0].key.clone(),
            current_version: "2".into(),
            pinned: true,
        }];
        assert!(!app.can_upgrade_all());
        app.updates[0].pinned = false;
        let _ = app.update(Message::Propose(Action::UpgradeAll));
        assert!(matches!(
            app.confirmation,
            Some(Confirmation::Package(Action::UpgradeAll))
        ));
        assert!(!app.busy());
        let _ = app.update(Message::Cancel);
        assert!(app.confirmation.is_none());
    }

    #[test]
    fn a_failed_bulk_exit_reconciles_remaining_updates_without_inventing_package_success() {
        let app = app();
        let mut operation = failed_upgrade(&app);
        operation.action = Action::UpgradeAll;
        operation.target_version = None;
        operation.reconcile(&[], &Err("cannot verify".into()));
        assert_eq!(operation.effect, Effect::Unchecked);
        operation.reconcile(
            &[],
            &Ok(vec![Outdated {
                key: app.packages[0].key.clone(),
                current_version: "2".into(),
                pinned: false,
            }]),
        );
        assert_eq!(operation.effect, Effect::NotApplied);
        operation.reconcile(&[], &Ok(vec![]));
        assert_eq!(operation.effect, Effect::Applied);
        assert!(matches!(operation.result, Some(Err(_))));
    }
    #[test]
    fn background_refresh_allows_preview_but_never_executes_before_verification() {
        let mut app = app();
        app.inventory_fresh = true;
        app.inventory_loading = true;
        app.updates_fresh = true;
        app.updates = vec![Outdated {
            key: app.packages[0].key.clone(),
            current_version: "2".into(),
            pinned: false,
        }];
        let _ = app.update(Message::Propose(Action::UpgradeAll));
        assert!(app.confirmation.is_some());
        let _ = app.update(Message::Confirm);
        assert!(app.confirmation.is_some());
        assert!(app.operation.is_none());
        assert!(!app.busy());
        let _ = app.update(Message::Cancel);
        assert!(app.confirmation.is_none());
    }
}
