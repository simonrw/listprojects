use std::{
    ffi::OsString,
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

    /// Assume the project is given on the command line and activate it directly
    #[clap(short, long)]
    path: Option<String>,
}

fn activation_name(path: impl AsRef<Path>) -> eyre::Result<String> {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            eyre::eyre!(
                "project path has no usable final component: {}",
                path.as_ref().display()
            )
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    Herdr,
    Tmux,
}

fn select_backend(herdr_env: Option<&str>, _tmux_env: Option<&str>) -> Backend {
    if herdr_env == Some("1") {
        Backend::Herdr
    } else {
        Backend::Tmux
    }
}

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
}

trait CommandRunner {
    fn output(&self, program: &str, args: &[OsString]) -> eyre::Result<CommandOutput>;
    fn exec(&self, program: &str, args: &[OsString]) -> eyre::Result<()>;
}

struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn output(&self, program: &str, args: &[OsString]) -> eyre::Result<CommandOutput> {
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .wrap_err_with(|| format!("running `{program}`"))?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: output.stdout,
        })
    }

    fn exec(&self, program: &str, args: &[OsString]) -> eyre::Result<()> {
        let error = std::process::Command::new(program).args(args).exec();
        Err(error).wrap_err_with(|| format!("replacing process with `{program}`"))
    }
}

fn command_args<const N: usize>(args: [&str; N]) -> Vec<OsString> {
    args.into_iter().map(OsString::from).collect()
}

fn activate_project<R: CommandRunner>(
    path: &Path,
    herdr_env: Option<&str>,
    tmux_env: Option<&str>,
    runner: &R,
) -> eyre::Result<()> {
    let name = activation_name(path)?;
    match select_backend(herdr_env, tmux_env) {
        Backend::Herdr => activate_herdr(path, &name, runner),
        Backend::Tmux => activate_tmux(path, &name, tmux_env.is_some(), runner),
    }
}

fn activate_herdr<R: CommandRunner>(path: &Path, name: &str, runner: &R) -> eyre::Result<()> {
    let list_args = command_args(["workspace", "list"]);
    let output = runner
        .output("herdr", &list_args)
        .wrap_err("listing Herdr workspaces")?;
    eyre::ensure!(output.success, "`herdr workspace list` failed");

    if let Some(workspace_id) = matching_herdr_workspace(&output.stdout, name)? {
        let args = command_args(["workspace", "focus", workspace_id.as_str()]);
        let output = runner
            .output("herdr", &args)
            .wrap_err_with(|| format!("focusing Herdr workspace `{workspace_id}`"))?;
        eyre::ensure!(
            output.success,
            "`herdr workspace focus {workspace_id}` failed"
        );
    } else {
        let args = vec![
            "workspace".into(),
            "create".into(),
            "--cwd".into(),
            path.as_os_str().to_owned(),
            "--label".into(),
            name.into(),
            "--focus".into(),
        ];
        let output = runner
            .output("herdr", &args)
            .wrap_err_with(|| format!("creating Herdr workspace `{name}`"))?;
        eyre::ensure!(
            output.success,
            "`herdr workspace create` failed for `{name}`"
        );
    }

    Ok(())
}

fn matching_herdr_workspace(json: &[u8], name: &str) -> eyre::Result<Option<String>> {
    let response: serde_json::Value =
        serde_json::from_slice(json).wrap_err("parsing `herdr workspace list` response as JSON")?;
    let workspaces = response
        .get("result")
        .and_then(|result| result.get("workspaces"))
        .and_then(serde_json::Value::as_array)
        .ok_or_eyre("`herdr workspace list` response is missing `result.workspaces`")?;

    let mut matches = Vec::new();
    for (index, workspace) in workspaces.iter().enumerate() {
        let label = workspace
            .get("label")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| eyre::eyre!("workspace {index} is missing string field `label`"))?;
        let number = workspace
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| eyre::eyre!("workspace {index} is missing unsigned field `number`"))?;
        let workspace_id = workspace
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                eyre::eyre!("workspace {index} is missing string field `workspace_id`")
            })?;

        if label == name {
            matches.push((number, workspace_id.to_owned()));
        }
    }

    Ok(matches
        .into_iter()
        .min_by_key(|(number, _)| *number)
        .map(|(_, workspace_id)| workspace_id))
}

fn activate_tmux<R: CommandRunner>(
    path: &Path,
    name: &str,
    in_tmux: bool,
    runner: &R,
) -> eyre::Result<()> {
    let has_args = command_args(["has-session", "-t", name]);
    let session_exists = runner
        .output("tmux", &has_args)
        .wrap_err_with(|| format!("checking whether tmux session `{name}` exists"))?
        .success;

    if !session_exists {
        let create_args = vec![
            "new-session".into(),
            "-d".into(),
            "-s".into(),
            name.into(),
            "-c".into(),
            path.as_os_str().to_owned(),
        ];
        let output = runner
            .output("tmux", &create_args)
            .wrap_err_with(|| format!("creating tmux session `{name}`"))?;
        eyre::ensure!(output.success, "creating tmux session `{name}` failed");
    }

    let action_args = if in_tmux {
        command_args(["switch-client", "-t", name])
    } else {
        command_args(["attach-session", "-t", name])
    };
    runner.exec("tmux", &action_args).wrap_err_with(|| {
        if in_tmux {
            format!("switching to tmux session `{name}`")
        } else {
            format!("attaching to tmux session `{name}`")
        }
    })
}

fn expand_user(given: impl AsRef<str>) -> eyre::Result<PathBuf> {
    let given = given.as_ref();
    if !given.contains('~') {
        return Ok(PathBuf::from(given));
    }

    let home_dir = std::env::home_dir().ok_or_eyre("No home dir found")?;
    let s = given.replace('~', &home_dir.display().to_string());
    Ok(PathBuf::from(s))
}

fn activate_from_environment(path: &Path) -> eyre::Result<()> {
    let herdr_env = std::env::var("HERDR_ENV").ok();
    let tmux_env = std::env::var("TMUX").ok();
    activate_project(
        path,
        herdr_env.as_deref(),
        tmux_env.as_deref(),
        &SystemCommandRunner,
    )
}

fn project_root_from_marker(path: &Path) -> Option<&Path> {
    let is_project_marker = path.file_name().is_some_and(|name| {
        (name == ".git" && (path.is_dir() || path.is_file())) || (name == ".jj" && path.is_dir())
    });
    if !is_project_marker {
        return None;
    }

    path.parent()
}

fn worktree_repo_root(path: &Path) -> Option<PathBuf> {
    let git_file = path.join(".git");
    if !git_file.is_file() {
        return None;
    }

    let git_file_contents = std::fs::read_to_string(git_file).ok()?;
    let git_dir = PathBuf::from(git_file_contents.strip_prefix("gitdir:")?.trim());
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        path.join(git_dir)
    };

    let common_dir = PathBuf::from(
        std::fs::read_to_string(git_dir.join("commondir"))
            .ok()?
            .trim(),
    );
    let common_dir = if common_dir.is_absolute() {
        common_dir
    } else {
        git_dir.join(common_dir)
    }
    .canonicalize()
    .ok()?;

    if common_dir.file_name().is_some_and(|name| name == ".git") {
        common_dir.parent().map(Path::to_path_buf)
    } else {
        Some(common_dir)
    }
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
        activate_from_environment(&full_path)?;
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

                        if path.is_dir()
                            && (path.ends_with(".venv")
                                || path.ends_with("node_modules")
                                || path.ends_with("venv")
                                || path.ends_with("__pycache__")
                                || path.extension().is_some_and(|ext| ext == "jj"))
                        {
                            return WalkState::Skip;
                        }

                        let Some(path) = project_root_from_marker(path) else {
                            return WalkState::Continue;
                        };

                        let pb = path.to_path_buf();
                        if cache.lock().unwrap().add_to_cache(pb.clone()) {
                            let item: Arc<dyn SkimItem> = Arc::new(SelectablePath::new(pb));
                            let _ = tx.send(item);
                        }
                    }
                    WalkState::Continue
                }
            })
        });
    });

    if args.list {
        for item in rx {
            let path = (*item).as_any().downcast_ref::<SelectablePath>().unwrap();
            println!("{}", path.display_text());
        }

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

    activate_from_environment(chosen_path)?;
    Ok(())
}

#[derive(Debug)]
struct SelectablePath {
    path: PathBuf,
    repo_root: Option<PathBuf>,
}

impl SelectablePath {
    fn new(path: PathBuf) -> Self {
        let repo_root = worktree_repo_root(&path);
        Self { path, repo_root }
    }

    fn display_text(&self) -> String {
        match &self.repo_root {
            Some(repo_root) => {
                format!("{} (repo: {})", self.path.display(), repo_root.display())
            }
            None => self.path.display().to_string(),
        }
    }
}

impl SkimItem for SelectablePath {
    fn text(&self) -> Cow<'_, str> {
        Cow::Owned(self.display_text())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;

    const WORKSPACES: &str = r#"{
        "result": {
            "workspaces": [
                {"number": 7, "workspace_id": "other", "label": "other-project"},
                {"number": 9, "workspace_id": "later", "label": "project"},
                {"number": 2, "workspace_id": "earlier", "label": "project"}
            ]
        }
    }"#;

    #[derive(Debug, Eq, PartialEq)]
    enum Call {
        Output(String, Vec<OsString>),
        Exec(String, Vec<OsString>),
    }

    struct FakeRunner {
        outputs: Mutex<VecDeque<CommandOutput>>,
        calls: Mutex<Vec<Call>>,
    }

    impl FakeRunner {
        fn new(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
            Self {
                outputs: Mutex::new(outputs.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn success(stdout: impl Into<Vec<u8>>) -> CommandOutput {
            CommandOutput {
                success: true,
                stdout: stdout.into(),
            }
        }

        fn failure() -> CommandOutput {
            CommandOutput {
                success: false,
                stdout: Vec::new(),
            }
        }

        fn calls(&self) -> Vec<Call> {
            std::mem::take(&mut *self.calls.lock().unwrap())
        }
    }

    impl CommandRunner for FakeRunner {
        fn output(&self, program: &str, args: &[OsString]) -> eyre::Result<CommandOutput> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Output(program.to_owned(), args.to_vec()));
            self.outputs
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_eyre("no fake output queued")
        }

        fn exec(&self, program: &str, args: &[OsString]) -> eyre::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Exec(program.to_owned(), args.to_vec()));
            Ok(())
        }
    }

    #[test]
    fn activation_names_are_exact_final_path_components() {
        assert_eq!(
            activation_name("/Users/simon/work/localstack/localstack-pro").unwrap(),
            "localstack-pro"
        );
        assert_eq!(
            activation_name("/tmp/project.with.dots").unwrap(),
            "project.with.dots"
        );
        assert_eq!(
            activation_name("/tmp/project with spaces").unwrap(),
            "project with spaces"
        );
        assert_eq!(
            activation_name("/one/shared").unwrap(),
            activation_name("/two/shared").unwrap()
        );
        assert!(activation_name("/").is_err());
        assert!(activation_name("").is_err());
    }

    #[test]
    fn git_worktree_marker_file_identifies_project_root() {
        let temp_root = std::env::temp_dir().join(format!(
            "listprojects-worktree-marker-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let marker = temp_root.join(".git");
        std::fs::write(&marker, "gitdir: /tmp/main/.git/worktrees/example\n").unwrap();

        assert_eq!(project_root_from_marker(&marker), Some(temp_root.as_path()));

        std::fs::remove_file(marker).unwrap();
        std::fs::remove_dir(temp_root).unwrap();
    }

    #[test]
    fn git_worktree_display_names_root_repository() {
        let temp_root = std::env::temp_dir().join(format!(
            "listprojects-worktree-display-{}",
            std::process::id()
        ));
        let repo_root = temp_root.join("repo");
        let git_dir = repo_root.join(".git/worktrees/example");
        let worktree_root = temp_root.join("worktree");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::create_dir_all(&worktree_root).unwrap();
        let expected_repo_root = repo_root.canonicalize().unwrap();
        std::fs::write(git_dir.join("commondir"), "../..\n").unwrap();
        std::fs::write(
            worktree_root.join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .unwrap();

        let item = SelectablePath::new(worktree_root.clone());

        assert_eq!(item.repo_root, Some(expected_repo_root.clone()));
        assert_eq!(
            item.display_text(),
            format!(
                "{} (repo: {})",
                worktree_root.display(),
                expected_repo_root.display()
            )
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn herdr_is_selected_only_for_exact_indicator_and_precedes_tmux() {
        assert_eq!(select_backend(Some("1"), None), Backend::Herdr);
        assert_eq!(select_backend(Some("1"), Some("tmux")), Backend::Herdr);
        assert_eq!(select_backend(None, Some("tmux")), Backend::Tmux);
        assert_eq!(select_backend(Some("0"), Some("tmux")), Backend::Tmux);
        assert_eq!(select_backend(Some("true"), None), Backend::Tmux);
    }

    #[test]
    fn workspace_matching_is_exact_and_uses_lowest_number() {
        assert_eq!(
            matching_herdr_workspace(WORKSPACES.as_bytes(), "project").unwrap(),
            Some("earlier".into())
        );
        assert_eq!(
            matching_herdr_workspace(WORKSPACES.as_bytes(), "proj").unwrap(),
            None
        );
        assert_eq!(
            matching_herdr_workspace(WORKSPACES.as_bytes(), "PROJECT").unwrap(),
            None
        );
    }

    #[test]
    fn malformed_workspace_responses_have_contextual_errors() {
        assert!(
            matching_herdr_workspace(b"not json", "project")
                .unwrap_err()
                .to_string()
                .contains("parsing")
        );
        assert!(
            matching_herdr_workspace(br#"{"result": {}}"#, "project")
                .unwrap_err()
                .to_string()
                .contains("result.workspaces")
        );
        assert!(
            matching_herdr_workspace(
                br#"{"result":{"workspaces":[{"label":"project"}]}}"#,
                "project"
            )
            .unwrap_err()
            .to_string()
            .contains("number")
        );
    }

    #[test]
    fn matching_herdr_workspace_is_focused_without_creation() {
        let runner = FakeRunner::new([FakeRunner::success(WORKSPACES), FakeRunner::success([])]);
        activate_project(Path::new("/tmp/project"), Some("1"), Some("tmux"), &runner).unwrap();

        assert_eq!(
            runner.calls(),
            vec![
                Call::Output("herdr".into(), command_args(["workspace", "list"])),
                Call::Output(
                    "herdr".into(),
                    command_args(["workspace", "focus", "earlier"])
                ),
            ]
        );
    }

    #[test]
    fn missing_herdr_workspace_is_created_and_focused() {
        let runner = FakeRunner::new([
            FakeRunner::success(br#"{"result":{"workspaces":[]}}"#.as_slice()),
            FakeRunner::success([]),
        ]);
        activate_project(Path::new("/tmp/my project"), Some("1"), None, &runner).unwrap();

        assert_eq!(
            runner.calls(),
            vec![
                Call::Output("herdr".into(), command_args(["workspace", "list"])),
                Call::Output(
                    "herdr".into(),
                    vec![
                        "workspace".into(),
                        "create".into(),
                        "--cwd".into(),
                        "/tmp/my project".into(),
                        "--label".into(),
                        "my project".into(),
                        "--focus".into(),
                    ]
                ),
            ]
        );
    }

    #[test]
    fn herdr_failures_do_not_fall_back_to_tmux() {
        let list_failure = FakeRunner::new([FakeRunner::failure()]);
        assert!(
            activate_project(
                Path::new("/tmp/project"),
                Some("1"),
                Some("tmux"),
                &list_failure
            )
            .is_err()
        );
        assert_eq!(list_failure.calls().len(), 1);

        let malformed = FakeRunner::new([FakeRunner::success("not json")]);
        assert!(
            activate_project(
                Path::new("/tmp/project"),
                Some("1"),
                Some("tmux"),
                &malformed
            )
            .is_err()
        );
        assert_eq!(malformed.calls().len(), 1);

        let focus_failure =
            FakeRunner::new([FakeRunner::success(WORKSPACES), FakeRunner::failure()]);
        assert!(
            activate_project(
                Path::new("/tmp/project"),
                Some("1"),
                Some("tmux"),
                &focus_failure
            )
            .is_err()
        );
        assert!(focus_failure.calls().iter().all(|call| !matches!(call, Call::Output(program, _) | Call::Exec(program, _) if program == "tmux")));
    }

    #[test]
    fn tmux_reuses_or_creates_then_switches_inside_tmux() {
        let existing = FakeRunner::new([FakeRunner::success([])]);
        activate_project(Path::new("/tmp/project"), None, Some("tmux"), &existing).unwrap();
        assert_eq!(
            existing.calls(),
            vec![
                Call::Output(
                    "tmux".into(),
                    command_args(["has-session", "-t", "project"])
                ),
                Call::Exec(
                    "tmux".into(),
                    command_args(["switch-client", "-t", "project"])
                ),
            ]
        );

        let missing = FakeRunner::new([FakeRunner::failure(), FakeRunner::success([])]);
        activate_project(Path::new("/tmp/project"), None, Some("tmux"), &missing).unwrap();
        assert_eq!(
            missing.calls(),
            vec![
                Call::Output(
                    "tmux".into(),
                    command_args(["has-session", "-t", "project"])
                ),
                Call::Output(
                    "tmux".into(),
                    vec![
                        "new-session".into(),
                        "-d".into(),
                        "-s".into(),
                        "project".into(),
                        "-c".into(),
                        "/tmp/project".into()
                    ]
                ),
                Call::Exec(
                    "tmux".into(),
                    command_args(["switch-client", "-t", "project"])
                ),
            ]
        );
    }

    #[test]
    fn tmux_reuses_or_creates_then_attaches_outside_tmux() {
        for (outputs, creates) in [
            (vec![FakeRunner::success([])], false),
            (vec![FakeRunner::failure(), FakeRunner::success([])], true),
        ] {
            let runner = FakeRunner::new(outputs);
            activate_project(Path::new("/tmp/project.with.dots"), None, None, &runner).unwrap();
            let calls = runner.calls();
            assert_eq!(
                calls.last(),
                Some(&Call::Exec(
                    "tmux".into(),
                    command_args(["attach-session", "-t", "project.with.dots"])
                ))
            );
            assert_eq!(calls.iter().any(|call| matches!(call, Call::Output(_, args) if args.first() == Some(&OsString::from("new-session")))), creates);
        }
    }

    #[test]
    fn tmux_command_failures_are_returned() {
        let runner = FakeRunner::new([FakeRunner::failure(), FakeRunner::failure()]);
        let error = activate_project(Path::new("/tmp/project"), None, None, &runner).unwrap_err();
        assert!(error.to_string().contains("creating tmux session"));
    }
}
