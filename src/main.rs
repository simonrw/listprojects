use std::{
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    sync::Mutex,
};

use clap::Parser;
use color_eyre::eyre::{self, Context, OptionExt};
use dark_light::Mode;
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

    /// Clear the cache before running
    #[clap(short, long)]
    clear: bool,

    /// Non-interactive mode: print all found directories to stdout
    #[clap(short, long)]
    list: bool,

    /// Assume the project is given on the command line and just create/reuse a tmux session
    #[clap(short, long)]
    path: Option<String>,
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
        let status = std::process::Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &self.session_name,
                "-c",
                &self.path.display().to_string(),
            ])
            .status()
            .wrap_err("creating new session")?;

        eyre::ensure!(status.success(), "creating new session failed");
        Ok(())
    }

    fn attach_session(&self) -> std::io::Error {
        std::process::Command::new("tmux")
            .args(["attach-session", "-t", &self.session_name])
            .exec()
    }
}

fn expand_user(given: impl AsRef<str>) -> eyre::Result<PathBuf> {
    let given = given.as_ref();
    if !given.contains("~") {
        return Ok(PathBuf::from(given));
    }

    let home_dir = std::env::home_dir().ok_or_eyre("No home dir found")?;
    let s = given.replace("~", &home_dir.display().to_string());
    Ok(PathBuf::from(s))
}

fn main() -> eyre::Result<()> {
    color_eyre::install().wrap_err("Installing color-eyre handler")?;
    let args = Args::parse();

    let cache = Arc::new(Mutex::new(Cache::new()));
    if args.clear
        && let Err(_e) = cache.lock().unwrap().clear()
    {
        todo!()
    };

    // shortcut - if the project path is specified on the command line then just switch to that
    // project
    if let Some(path) = args.path {
        let full_path = expand_user(path)
            .context("failed to expand ~ for user directory")?
            .canonicalize()
            .wrap_err("Given path does not exist")?;
        {
            let mut c = cache.lock().unwrap();
            c.record_visit(&full_path);
            c.save().unwrap();
        }
        let session = Tmux::new(full_path);
        session.activate();
        return Ok(());
    }

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

    if args.list {
        // Non-interactive mode: print directories as they are discovered
        for item in rx {
            let path = (*item).as_any().downcast_ref::<SelectablePath>().unwrap();
            println!("{}", path.path.display());
        }

        // Save cache before exiting
        cache.lock().unwrap().save().unwrap();

        return Ok(());
    }

    let system_colour_theme = dark_light::detect().unwrap_or(Mode::Dark);
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

    {
        let mut c = cache.lock().unwrap();
        c.record_visit(chosen_path);
        c.save().unwrap();
    }

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
        assert_eq!(compute_session_name("/Users/simon/work/bar"), "work/bar");
        assert_eq!(
            compute_session_name("/home/user/projects/my-project"),
            "projects/my-project"
        );
        assert_eq!(
            compute_session_name("/Users/simon/dev/deeply/nested/repo"),
            "nested/repo"
        );
        assert_eq!(compute_session_name("/tmp/a/b"), "a/b");
        assert_eq!(
            compute_session_name("/Users/simon/dev/project.with"),
            "dev/project.with"
        );
        assert_eq!(
            compute_session_name("/Users/simon/dev/my-dashed-project"),
            "dev/my-dashed-project"
        );
        assert_eq!(
            compute_session_name("/Users/simon/dev/under_score_project"),
            "dev/under_score_project"
        );
    }
}
