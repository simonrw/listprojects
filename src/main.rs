use std::{os::unix::process::CommandExt, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{self, Context, OptionExt};
use ignore::{WalkBuilder, WalkState};
use skim::prelude::*;

#[derive(Parser)]
struct Args {
    /// Root paths to search (default: ~/dev ~/work)
    root: Option<Vec<PathBuf>>,
}

struct Tmux {
    path: PathBuf,
    session_name: String,
}

impl Tmux {
    fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            path: path.clone(),
            session_name: format!(
                "{}/{}",
                path.parent().unwrap().display(),
                path.file_name().unwrap().to_string_lossy()
            ),
        }
    }

    fn activate(&self) -> std::io::Error {
        if Self::in_tmux_session() {
            if self.session_exists().unwrap() {
                self.switch_session()
            } else {
                self.create_session().expect("creating session");
                self.switch_session()
            }
        } else {
            self.create_session().expect("creating session");
            self.attach_session()
        }
    }

    fn in_tmux_session() -> bool {
        std::env::var("TMUX").is_ok()
    }

    fn session_exists(&self) -> eyre::Result<bool> {
        let output = std::process::Command::new("tmux")
            .arg("has-session")
            .arg("-t")
            .arg(&self.session_name)
            .output()
            .wrap_err("Checking if tmux session exists")?;

        Ok(output.status.success())
    }

    fn switch_session(&self) -> std::io::Error {
        std::process::Command::new("tmux")
            .args(["switch-client", "-t", &self.session_name])
            .exec()
    }

    fn create_session(&self) -> eyre::Result<()> {
        std::process::Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &self.session_name,
                "-c",
                &self.path.display().to_string(),
            ])
            .spawn()
            .wrap_err("creating new session")?;
        Ok(())
    }

    fn attach_session(&self) -> std::io::Error {
        std::process::Command::new("tmux")
            .args(["attach-session", "-t", &self.session_name])
            .exec()
    }
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
    .build_parallel();

    let (tx, rx) = unbounded();

    std::thread::spawn(move || {
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
            let item = (*item).as_any().downcast_ref::<SelectablePath>().unwrap();
            item.path.clone()
        })
        .collect::<Vec<_>>();
    let chosen_path = items.first().unwrap();
    let session = Tmux::new(chosen_path);
    session.activate();

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
