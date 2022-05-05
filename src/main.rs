use eyre::{Result, WrapErr};
use skim::SkimOptions;
use std::{
    borrow::Cow,
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tmux_interface::TmuxCommand;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long)]
    clear: bool,

    #[clap(long)]
    config: Option<PathBuf>,
}

// Cache types

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct ProjectPath {
    full_path: String,
    session_name: String,
}

impl skim::SkimItem for ProjectPath {
    fn text(&self) -> std::borrow::Cow<str> {
        std::borrow::Cow::Borrowed(&self.full_path)
    }
}

#[derive(Debug, Clone)]
struct Cache {
    inner: Arc<RwLock<CacheInner>>,
    loc: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct CacheInner {
    paths: HashSet<ProjectPath>,
}

impl Cache {
    fn new(clear: bool) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("~/.cache"))
            .join("project");
        std::fs::create_dir_all(&cache_dir).wrap_err("creating cache directory")?;
        let cache_file = cache_dir.join("config.json");

        match std::fs::read_to_string(&cache_file) {
            Ok(txt) => {
                let cache_inner: CacheInner = serde_json::from_str(&txt)?;
                let cache = Cache {
                    inner: Arc::new(RwLock::new(cache_inner)),
                    loc: cache_file,
                };
                if clear {
                    cache.clear();
                }
                Ok(cache)
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    let inner = CacheInner {
                        paths: HashSet::new(),
                    };
                    let cache = Cache {
                        inner: Arc::new(RwLock::new(inner)),
                        loc: cache_file,
                    };
                    cache.write().wrap_err("writing cache")?;
                    Ok(cache)
                }
                _ => return Err(eyre::eyre!("IO error: {:?}", e)),
            },
        }
    }

    fn write(&self) -> Result<()> {
        let mut f = std::fs::File::create(&self.loc).wrap_err("creating cache file")?;
        let lock = self.inner.read().unwrap();
        serde_json::to_writer(&mut f, &*lock).wrap_err("writing cache file")?;
        Ok(())
    }

    fn clear(&self) {
        let mut lock = self.inner.write().unwrap();
        lock.paths.clear();
    }

    fn initial_paths(&self) -> Vec<ProjectPath> {
        let lock = self.inner.read().unwrap();
        lock.paths.iter().cloned().collect()
    }

    fn add(&self, value: ProjectPath) -> CacheState {
        let mut lock = self.inner.write().unwrap();
        let inserted = lock.paths.insert(value);
        if inserted {
            CacheState::Missing
        } else {
            CacheState::Found
        }
    }
}

enum CacheState {
    Missing,
    Found,
}

impl Drop for Cache {
    fn drop(&mut self) {
        if let Err(e) = self.write() {
            log::warn!("saving cache: {:?}", e);
        }
    }
}

// config types
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    root_dirs: Vec<RootDir>,
}

fn expand_path<'de, D>(deserializer: D) -> std::result::Result<PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = serde::Deserialize::deserialize(deserializer)?;
    let transformed = shellexpand::tilde(s);
    match transformed {
        Cow::Borrowed(s) => Ok(PathBuf::from(s)),
        Cow::Owned(s) => Ok(PathBuf::from(s)),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RootDir {
    #[serde(deserialize_with = "expand_path")]
    path: PathBuf,
    prefix: Option<String>,
}

impl Config {
    fn open(config_path: PathBuf) -> Result<Self> {
        let config_txt = std::fs::read_to_string(&config_path).wrap_err("reading config file")?;
        let config: Config = toml::from_str(&config_txt).wrap_err("parsing config file")?;
        Ok(config)
    }
}

struct Tmux<'a> {
    path: &'a ProjectPath,
    client: TmuxCommand<'a>,
}

impl<'a> Tmux<'a> {
    fn new(item: &'a ProjectPath) -> Self {
        let client = TmuxCommand::new();
        Self { path: item, client }
    }

    fn create(&self) -> Result<()> {
        if self.is_running() {
            if !self.session_exists()? {
                self.create_session().wrap_err("creating session")?;
            }
            self.switch_client().wrap_err("switching client")?;
        } else if self.session_exists()? {
            self.join().wrap_err("joining session")?;
        } else {
            self.create_session().wrap_err("creating session")?;
            self.join().wrap_err("joining session")?;
        }

        Ok(())
    }

    fn join(&self) -> Result<()> {
        self.client
            .attach_session()
            .target_session(&self.path.session_name)
            .output()?;
        Ok(())
    }

    fn create_session(&self) -> Result<()> {
        self.client
            .new_session()
            .detached()
            .start_directory(&self.path.full_path)
            .session_name(&self.path.session_name)
            .output()?;
        Ok(())
    }

    fn session_exists(&self) -> Result<bool> {
        let res = self
            .client
            .has_session()
            .target_session(&self.path.session_name)
            .output()
            .wrap_err("checking if session exists")?;
        Ok(res.status().success())
    }

    fn is_running(&self) -> bool {
        std::env::var("TMUX").is_ok()
    }

    fn switch_client(&self) -> Result<()> {
        self.client
            .switch_client()
            .target_session(&self.path.session_name)
            .output()?;
        Ok(())
    }
}

fn compute_session_name(full_path_str: &str, dir_path_str: &str) -> String {
    let dir_removed = full_path_str
        .strip_prefix(dir_path_str)
        .unwrap_or(full_path_str);
    let leading_slash_removed = dir_removed.strip_prefix('/').unwrap_or(dir_removed);
    leading_slash_removed.to_owned()
}

trait SkimOptionsFromEnv {
    fn from_env() -> Self
    where
        Self: Sized;
}

impl SkimOptionsFromEnv for SkimOptions<'_> {
    fn from_env() -> Self
    where
        Self: Sized,
    {
        let colour = std::env::var("SKIM_DEFAULT_OPTIONS")
            .map(|default_options| {
                if default_options.contains("light") {
                    Some("light,matched_bg:-1")
                } else if default_options.contains("dark") {
                    Some("dark,matched_bg:-1")
                } else {
                    None
                }
            })
            .unwrap_or(None);

        skim::SkimOptions {
            color: colour,
            tiebreak: Some("begin".to_string()),
            no_mouse: true,
            tabstop: Some("4"),
            inline_info: true,
            ..Default::default()
        }
    }
}

fn main() -> Result<()> {
    color_eyre::install().unwrap();

    let args = Args::parse();

    let config_path = args.config.unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("project")
            .join("config.toml")
    });

    let cfg = Config::open(config_path).wrap_err("opening config")?;

    let cache = Cache::new(args.clear).wrap_err("creating cache")?;
    let (tx, rx): (skim::SkimItemSender, skim::SkimItemReceiver) = crossbeam_channel::unbounded();
    let project_paths = cache.initial_paths();
    let initial_tx = tx.clone();
    for path in project_paths {
        let _ = initial_tx.send(Arc::new(path));
    }

    // spawn background thread which updates the cache
    std::thread::spawn(move || {
        // walk the file system with the given config and update the cache
        for dir in cfg.root_dirs {
            let walker = ignore::WalkBuilder::new(dir.path.clone()).build();
            let dir_path_str = dir.path.to_str().unwrap();
            let matches = walker
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| e.path().join(".git").is_dir());
            for result in matches {
                let path = result.into_path();
                let full_path_str = path.to_str().unwrap().to_string();
                let session_name = compute_session_name(&full_path_str, dir_path_str);

                let project_path = ProjectPath {
                    full_path: full_path_str,
                    session_name,
                };

                if let CacheState::Missing = cache.add(project_path.clone()) {
                    let _ = tx.send(Arc::new(project_path));
                }
            }
        }
    });

    let options = skim::SkimOptions::from_env();
    if let Some(result) = skim::Skim::run_with(&options, Some(rx)) {
        if result.is_abort {
            return Ok(());
        }

        let item = &result.selected_items[0];
        // we know this is a ProjectPath, so downcast accordingly
        let item: &ProjectPath = item.as_any().downcast_ref().unwrap();

        let session = Tmux::new(item);
        session.create().wrap_err("creating tmux session")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_name() {
        let full_path = "/Users/user/work/project/a/b/c";
        let dir_path_str = "/Users/user/work";

        assert_eq!(
            compute_session_name(full_path, dir_path_str),
            "project/a/b/c"
        );
    }
}
