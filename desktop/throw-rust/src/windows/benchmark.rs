//! Opt-in Windows-only integration/measurement run. Never used during startup.
use super::{files, platform::*};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use throw_launcher::{config::Settings, files::FILE_CANDIDATES, index::Entry, search};
use windows::{
    core::HSTRING,
    Win32::System::{Com::*, Search::*},
};

pub unsafe fn run(root: &Path) -> Result<String, String> {
    let fixture = root.join("IndexedFiles");
    std::fs::create_dir_all(&fixture).map_err(|e| e.to_string())?;
    let marker = format!("ThrowFileProbe{}", std::process::id());
    for n in 0..1000 {
        std::fs::write(
            fixture.join(format!("{marker}-{n:04}.txt")),
            b"Throw search fixture",
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        fixture.join(format!("{marker}-O'Brien-é.txt")),
        b"Unicode fixture",
    )
    .map_err(|e| e.to_string())?;
    let manager: ISearchManager = CoCreateInstance(&CSearchManager, None, CLSCTX_LOCAL_SERVER)
        .map_err(|e| format!("Windows Search manager unavailable: {e}"))?;
    let catalog = manager
        .GetCatalog(windows::core::w!("SystemIndex"))
        .map_err(|e| e.to_string())?;
    let scope = catalog.GetCrawlScopeManager().map_err(|e| e.to_string())?;
    let url = HSTRING::from(format!(
        "file:///{}/",
        fixture.to_string_lossy().replace('\\', "/")
    ));
    scope
        .AddUserScopeRule(&url, true, true, 0)
        .map_err(|e| e.to_string())?;
    // Always remove the temporary indexing rule, including on a failed assertion.
    let result = (|| -> Result<String, String> {
        scope.SaveAll().map_err(|e| e.to_string())?;
        catalog
            .ReindexMatchingURLs(&HSTRING::from(format!("{}*", url)))
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if files::search(&format!("{marker}-O'Brien-é"))?.len() == 1 {
                break;
            }
            if Instant::now() >= deadline {
                return Err(
                    "Windows Search did not index the temporary fixture in 180 seconds.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let mut report = format!(
            "Windows Search integration + process-memory sample\n\
             Fixture: 1,001 temporary indexed text files; TOP {FILE_CANDIDATES}; 30 queries per app-index mode.\n\
             Includes native COM connection and query overhead, excludes 150ms UI debounce.\n\
             SearchIndexer service memory is external and NOT included. This is CI, not a desktop idle benchmark.\n\
             Before: {}\n", memory_report()?
        );
        for count in [5000, 20000] {
            let entries: Vec<_> = (0..count)
                .map(|n| Entry {
                    name: format!("ThrowFileProbe App {n}"),
                    path: root.join(format!("App{n}.lnk")),
                })
                .collect();
            let settings = Settings::default();
            report.push_str(&format!("{count} apps loaded: {}\n", memory_report()?));
            let mut times = Vec::new();
            for _ in 0..30 {
                let start = Instant::now();
                let results = files::search(&marker)?;
                if results.len() != FILE_CANDIDATES || results.iter().any(|e| !e.path.is_file()) {
                    return Err(
                        "Windows Search did not return bounded, existing local file results."
                            .into(),
                    );
                }
                let mut rows = search::query(&marker, &entries, &settings);
                throw_launcher::files::merge(&mut rows, &results, settings.max_results, false);
                if rows.len() > settings.max_results
                    || !rows
                        .iter()
                        .any(|r| matches!(r.action, search::Action::File(_)))
                {
                    return Err(
                        "Mixed app/file results were not bounded or files were hidden.".into(),
                    );
                }
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            let first = times[0];
            times.sort_by(f64::total_cmp);
            report.push_str(&format!(
                "{count} apps: first={first:.2}ms median={:.2}ms p95={:.2}ms max={:.2}ms\nAfter searches: {}\n",
                times[15], times[28], times[29], memory_report()?
            ));
        }
        report.push_str(&format!(
            "After dropping app index/results: {}\n",
            memory_report()?
        ));
        Ok(report)
    })();
    let cleanup = scope.RemoveScopeRule(&url).and_then(|_| scope.SaveAll());
    match (result, cleanup) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(format!(
            "Cannot remove temporary Windows Search scope: {error}"
        )),
    }
}
