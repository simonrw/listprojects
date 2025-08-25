use std::{
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    sync::Mutex,
};

use clap::Parser;
use color_eyre::eyre::{self, Context, OptionExt};
use ignore::{WalkBuilder, WalkState};
use skim::prelude::*;

use crate::disk_cache::Cache;

mod disk_cache;

/// List all projects
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Root paths to search (default: ~/dev ~/work)
    root: Option<Vec<PathBuf>>,
}

fn compute_session_name(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let mut iter = path.components().rev();
    let file = iter.next().unwrap().as_os_str().to_string_lossy();
    let parent = iter.next().unwrap().as_os_str().to_string_lossy();
    format!("{}/{}", parent, file)
}

#[derive(Debug)]
struct Tmux {
    path: PathBuf,
    session_name: String,
}

impl Tmux {
    fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            path: path.clone(),
            session_name: compute_session_name(path),
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

    let cache = Arc::new(Mutex::new(Cache::new()));

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
    .follow_links(false)
    .ignore(true)
    .git_ignore(true)
    .git_global(true)
    .git_exclude(true)
    .standard_filters(false)
    .build_parallel();

    let (tx, rx) = unbounded();

    cache.lock().unwrap().prepopulate_with(tx.clone());

    let background_cache = cache.clone();
    std::thread::spawn(move || {
        walker.run(|| {
            Box::new({
                let cache = background_cache.clone();
                let tx = tx.clone();

                move |entry| {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if !path.is_dir() {
                            return WalkState::Continue;
                        }

                        // skip common directories
                        if path.ends_with(".venv")
                            || path.ends_with("node_modules")
                            || path.ends_with("venv")
                            || path.ends_with("__pycache__")
                            || path.extension().is_some_and(|ext| ext == "jj")
                        {
                            return WalkState::Skip;
                        }

                        if !path.ends_with(".git") {
                            return WalkState::Continue;
                        }

                        // if path.display().to_string().contains(".git") {
                        //     return WalkState::Skip;
                        // }

                        let path = path.parent().unwrap();

                        let pb = path.to_path_buf();
                        if cache.lock().unwrap().add_to_cache(pb.clone()) {
                            let item: Arc<dyn SkimItem> =
                                Arc::new(SelectablePath { path: pb.clone() });
                            let _ = tx.send(item);
                        }
                    }
                    WalkState::Continue
                }
            })
        });
    });

    let system_colour_theme = dark_light::detect().wrap_err("detecting system colour theme")?;
    let options = SkimOptions {
        color: match system_colour_theme {
            dark_light::Mode::Dark => Some("dark".to_string()),
            _ => Some("light".to_string()),
        },
        ..Default::default()
    };

    let selected = Skim::run_with(&options, Some(rx)).ok_or_eyre("running fuzzy finder")?;

    // explicitly save the cache
    cache.lock().unwrap().save().unwrap();

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
    fn text(&self) -> Cow<'_, str> {
        Cow::Owned(self.path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::compute_session_name;

    #[test]
    fn test_compute_session_name() {
        assert_eq!(compute_session_name("/Users/simon/dev/foo"), "dev/foo");
    }
}
