use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::{self, Context, OptionExt};
use ignore::{WalkBuilder, WalkState};
use skim::prelude::*;

#[derive(Parser)]
struct Args {
    /// Root paths to search (default: ~/dev ~/work)
    root: Option<Vec<PathBuf>>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install().wrap_err("Installing color-eyre handler")?;
    let args = Args::parse();

    let home = dirs::home_dir().ok_or_else(|| eyre::eyre!("Calculating home directory"))?;
    let roots = args
        .root
        .unwrap_or_else(|| vec![home.join("dev"), home.join("work")]);

    let walker = if roots.len() == 1 {
        WalkBuilder::new(&roots[0])
    } else {
        let mut builder = WalkBuilder::new(&roots[0]);
        for root in roots.iter().skip(1) {
            builder.add(root);
        }
        builder
    }
    .standard_filters(false)
    // .hidden(true)
    .build_parallel();

    let (tx, rx) = unbounded();

    let handle = std::thread::spawn(move || {
        walker.run(|| {
            Box::new(|entry| {
                if let Ok(entry) = entry {
                    if !entry.path().is_dir() {
                        return WalkState::Continue;
                    }

                    if !entry.path().ends_with(".git") {
                        return WalkState::Continue;
                    }

                    let path = entry.path().parent().unwrap();

                    let item: Arc<dyn SkimItem> = Arc::new(SelectablePath {
                        path: path.to_path_buf(),
                    });

                    let _ = tx.send(item);
                }
                WalkState::Continue
            })
        });
    });

    let selected =
        Skim::run_with(&SkimOptions::default(), Some(rx)).ok_or_eyre("running fuzzy finder")?;

    if selected.is_abort {
        return Ok(());
    }

    let items = selected
        .selected_items
        .into_iter()
        .map(|item| {
            let item = item.as_any().downcast_ref::<SelectablePath>().unwrap();
            item.path.clone()
        })
        .collect::<Vec<_>>();
    dbg!(&items);

    Ok(())
}

#[derive(Debug)]
struct SelectablePath {
    path: PathBuf,
}

impl SkimItem for SelectablePath {
    fn text(&self) -> Cow<str> {
        Cow::Owned(self.path.display().to_string())
    }
}
