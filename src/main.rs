use skim::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opts {}

fn compute_short_name(p: impl AsRef<Path>) -> String {
    // XXX: So many clones
    let p = p.as_ref();
    let parts: PathBuf = p.components().rev().take(2).collect();
    let result: PathBuf = parts.components().rev().collect();
    result.to_str().unwrap().to_owned()
}

#[derive(Debug)]
struct Selectable {
    path: PathBuf,
    short_name: String,
}

impl SkimItem for Selectable {
    fn text(&self) -> std::borrow::Cow<str> {
        std::borrow::Cow::Borrowed(self.short_name.as_str())
    }
}

fn walk_directory(
    walker: ignore::Walk,
    results: crossbeam_channel::Sender<Arc<dyn SkimItem + 'static>>,
) {
    for every in walker {
        let every = every.unwrap();
        let path = every.path().to_owned();
        let short_name = compute_short_name(&path);
        let e: Arc<dyn SkimItem + 'static> = Arc::new(Selectable { short_name, path });
        results.send(e).unwrap();
    }

    drop(results);
}

fn main() {
    tracing_subscriber::fmt::init();
    let args = Opts::from_args();

    tracing::info!(?args, "starting");

    let dirs: Vec<_> = ["dev"]
        .iter()
        .map(|stem| PathBuf::from("/home/simon").join(stem))
        .collect();

    tracing::debug!(?dirs, "using directories");

    let mut builder = ignore::WalkBuilder::new(&dirs[0]);
    builder.max_depth(Some(1));
    builder.filter_entry(|e| e.path().is_dir());
    for dir in dirs.iter().skip(1) {
        builder.add(dir);
    }
    let walker = builder.build();

    let (tx, rx) = crossbeam_channel::bounded(100);
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .final_build()
        .unwrap();

    let handle = std::thread::spawn(move || walk_directory(walker, tx));
    let results = Skim::run_with(&options, Some(rx)).unwrap();
    handle.join().unwrap();

    if results.selected_items.is_empty() {
        return;
    }

    let item = Arc::clone(&results.selected_items[0]);
    let chosen: &Selectable = (*item).as_any().downcast_ref::<Selectable>().unwrap();
}
