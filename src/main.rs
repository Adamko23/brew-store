mod app;
mod assets;
mod brew;
mod catalog;
mod dependencies;
mod i18n;
mod model;
mod native;
mod search;
mod tools;
mod ui;
mod updater;

use app::App;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("Brew Store: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut language_override = None;
    let mut directory = catalog::data_dir();
    let mut diagnose = false;
    let mut benchmark = false;
    let mut theme_override = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("Brew Store {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--diagnose" => diagnose = true,
            "--benchmark-search" => benchmark = true,
            "--language" => {
                language_override = match args.next().as_deref() {
                    Some("en") => Some(i18n::Language::English),
                    Some("sk") => Some(i18n::Language::Slovak),
                    Some("auto") => None,
                    _ => return Err("--language: use auto, en or sk.".into()),
                }
            }
            "--theme" => {
                theme_override = match args.next().as_deref() {
                    Some("light") => Some(iced::theme::Mode::Light),
                    Some("dark") => Some(iced::theme::Mode::Dark),
                    Some("auto") => None,
                    _ => return Err("--theme: použi auto, light alebo dark.".into()),
                }
            }
            "--data-dir" => {
                directory = PathBuf::from(args.next().ok_or("--data-dir vyžaduje cestu.")?)
            }
            "--help" | "-h" => {
                println!(
                    "Brew Store\n\n  brew-store [--data-dir PATH] [--theme auto|light|dark] [--language auto|en|sk]\n  brew-store --diagnose [--data-dir PATH]\n  brew-store --benchmark-search [--data-dir PATH]\n  brew-store --version\n\nDiagnostics only read Homebrew state; they do not change packages."
                );
                return Ok(());
            }
            _ => return Err(format!("Neznámy argument: {arg}")),
        }
    }
    i18n::initialize(language_override);
    if diagnose {
        return tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(diagnostic(directory));
    }
    if benchmark {
        return tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(benchmark_search(directory));
    }
    let application = iced::application(
        move || {
            if theme_override.is_some() {
                App::boot_with_theme(directory.clone(), theme_override)
            } else {
                App::boot(directory.clone())
            }
        },
        App::update,
        ui::view,
    )
    .title("Brew Store")
    .default_font(iced::Font::with_name("System Font"))
    .theme(ui::app_theme)
    .subscription(App::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1180.0, 780.0),
        min_size: Some(iced::Size::new(1000.0, 640.0)),
        exit_on_close_request: false,
        #[cfg(target_os = "macos")]
        platform_specific: iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            // AppKit's native titlebar captures clicks in its own area. Keep
            // interactive Iced controls inside the actual content layout.
            fullsize_content_view: false,
        },
        ..Default::default()
    })
    .centered();
    let application = match std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        Ok(font) => application.font(font),
        Err(_) => application.default_font(iced::Font::with_name("Helvetica Neue")),
    };
    application.run().map_err(|e| e.to_string())
}

async fn benchmark_search(directory: PathBuf) -> Result<(), String> {
    let catalog = catalog::read_cache(&directory)
        .await?
        .ok_or("Najprv načítaj katalóg cez aplikáciu alebo --diagnose.")?;
    let mut samples = Vec::new();
    for _ in 0..50 {
        for query in ["firefox", "video", "git", "visual studio"] {
            let start = std::time::Instant::now();
            std::hint::black_box(search::find(
                &catalog.packages,
                app::Page::Search,
                app::Scope::All,
                query,
            ));
            samples.push(start.elapsed().as_micros());
        }
    }
    samples.sort();
    println!(
        "{} položiek, {} vyhľadávaní; p50={} µs, p95={} µs, max={} µs",
        catalog.packages.len(),
        samples.len(),
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() - 1]
    );
    Ok(())
}

async fn diagnostic(directory: PathBuf) -> Result<(), String> {
    let brew = brew::Brew::discover()?;
    let snapshot = brew.snapshot().await?;
    println!("Homebrew: {}", brew.executable.display());
    println!(
        "Inventár: {} nástrojov, {} aplikácií",
        snapshot
            .packages
            .iter()
            .filter(|p| p.key.kind == model::Kind::Formula)
            .count(),
        snapshot
            .packages
            .iter()
            .filter(|p| p.key.kind == model::Kind::Cask)
            .count()
    );
    match snapshot.outdated {
        Ok(ref updates) => println!("Dostupné aktualizácie: {}", updates.len()),
        Err(ref error) => return Err(error.clone()),
    }
    let catalog = match catalog::refresh(directory.clone()).await {
        Ok((catalog, warning)) => {
            if let Some(warning) = warning {
                eprintln!("{warning}");
            }
            catalog
        }
        Err(error) => {
            eprintln!("Sieť: {error}");
            catalog::read_cache(&directory)
                .await?
                .ok_or("Katalóg nie je dostupný ani v cache.")?
        }
    };
    let packages = model::merge(
        &catalog.packages,
        &snapshot.packages,
        &snapshot.outdated.unwrap_or_default(),
    );
    println!("Katalóg: {} položiek", catalog.packages.len());
    for query in ["firefox", "video", "git", "visual studio"] {
        println!(
            "Vyhľadávanie {query:?}: {} výsledkov",
            search::find(&packages, app::Page::Search, app::Scope::All, query).len()
        );
    }
    let graph = dependencies::InstalledGraph::build(&snapshot.packages);
    for package in snapshot
        .packages
        .iter()
        .filter(|p| p.token.contains("gstreamer"))
    {
        println!(
            "{} {}: priame potreby {}, priami používatelia {}, ďalší runtime {}, nepriami {}",
            package.title,
            package.installed_version(),
            graph.needs(&package.key).len(),
            graph.direct_users(&package.key).len(),
            graph.runtime_users(&package.key).len(),
            graph.indirect_users(&package.key).len()
        );
    }
    let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut jobs = tokio::task::JoinSet::new();
    for package in snapshot
        .packages
        .into_iter()
        .filter(|p| p.key.kind == model::Kind::Cask)
    {
        jobs.spawn(assets::inspect(
            package,
            snapshot.cellar.clone(),
            directory.clone(),
            gate.clone(),
        ));
    }
    let (mut icons, mut sizes) = (0, 0);
    while let Some(result) = jobs.join_next().await {
        let visual = result.map_err(|e| e.to_string())?;
        icons += usize::from(visual.icon.is_some());
        sizes += usize::from(visual.bytes.is_some());
    }
    println!("Lokálne ikony: {icons}; zmerané veľkosti aplikácií: {sizes}");
    Ok(())
}
