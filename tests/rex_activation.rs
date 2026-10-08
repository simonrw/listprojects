use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn rex_activation_through_cli() {
    let root = std::env::temp_dir().join(format!("listprojects-rex-{}", std::process::id()));
    let bin = root.join("bin");
    let project = root.join("my.repo: name");
    let log = root.join("commands");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let session_name = format!(
        "{}/my-repo-name",
        root.file_name().unwrap().to_str().unwrap()
    );

    for program in ["rex", "tmux", "herdr"] {
        let path = bin.join(program);
        fs::write(
            &path,
            format!(
                r#"#!/bin/sh
printf '%s\n' '{program}' "$@" '--end--' >> "$COMMAND_LOG"
[ '{program}' = rex ] || exit 99
[ "$1" != "$FAIL_ACTION" ] || exit 1
case "$1" in
    ls) printf '%s' "$LIST_RESPONSE" ;;
    new) printf '%s' "$NEW_RESPONSE" ;;
esac
"#
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let run = |backend: Option<&str>,
               herdr: Option<&str>,
               tmux: Option<&str>,
               rex: Option<&str>,
               list: &str,
               new: &str,
               fail: &str| {
        fs::write(&log, "").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_listprojects"));
        command
            .arg("--path")
            .arg(&project)
            .env("PATH", &bin)
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("HOME", &root)
            .env("COMMAND_LOG", &log)
            .env("LIST_RESPONSE", list)
            .env("NEW_RESPONSE", new)
            .env("FAIL_ACTION", fail);
        for (key, value) in [
            ("SESSION_BACKEND", backend),
            ("HERDR_ENV", herdr),
            ("TMUX", tmux),
            ("REX_SESSION", rex),
        ] {
            command.env_remove(key);
            if let Some(value) = value {
                command.env(key, value);
            }
        }
        command.output().unwrap()
    };

    let sessions_json = serde_json::json!({
        "sessions": [
            {"label": "my.repo: name", "session_id": "wrong"},
            {"label": session_name, "session_id": "existing"},
            {"label": session_name, "session_id": "later"}
        ]
    })
    .to_string();
    let sessions = sessions_json.as_str();
    let missing_id_json = serde_json::json!({"sessions": [{"label": session_name}]}).to_string();
    let missing_id = missing_id_json.as_str();
    for creates in [false, true] {
        for in_rex in [false, true] {
            for automatic in [false, true] {
                if automatic && !in_rex {
                    continue;
                }
                let output = run(
                    if automatic { None } else { Some("rex") },
                    if automatic { None } else { Some("1") },
                    if automatic { None } else { Some("tmux") },
                    if in_rex { Some("current") } else { None },
                    if creates {
                        r#"{"sessions":[]}"#
                    } else {
                        sessions
                    },
                    r#"{"session_id":"created"}"#,
                    "",
                );
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let mut expected = vec!["rex", "ls", "--json", "--end--"];
                if creates {
                    expected.extend([
                        "rex",
                        "new",
                        &session_name,
                        "--cwd",
                        project.to_str().unwrap(),
                        "--json",
                        "--end--",
                    ]);
                }
                let id = if creates { "created" } else { "existing" };
                let select_arg = format!("session_id={id}");
                if in_rex {
                    expected.extend(["rex", "do", "session.select", &select_arg, "--end--"]);
                } else {
                    expected.extend(["rex", "attach", id, "--end--"]);
                }
                assert_eq!(
                    fs::read_to_string(&log).unwrap(),
                    format!("{}\n", expected.join("\n"))
                );
            }
        }
    }

    for (list, new, fail, error) in [
        (sessions, "", "ls", "`rex ls --json` failed"),
        ("not json", "", "", "parsing `rex ls --json`"),
        ("{}", "", "", "missing `sessions`"),
        (r#"{"sessions":{}}"#, "", "", "missing `sessions`"),
        (
            r#"{"sessions":[{}]}"#,
            "",
            "",
            "missing string field `label`",
        ),
        (missing_id, "", "", "missing string field `session_id`"),
        (r#"{"sessions":[]}"#, "", "new", "creating Rex session"),
        (r#"{"sessions":[]}"#, "not json", "", "parsing `rex new`"),
        (
            r#"{"sessions":[]}"#,
            "{}",
            "",
            "missing string field `session_id`",
        ),
        (sessions, "", "attach", ""),
        (sessions, "", "do", ""),
    ] {
        let output = run(
            Some("rex"),
            None,
            None,
            if fail == "do" { Some("current") } else { None },
            list,
            new,
            fail,
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
        let calls = fs::read_to_string(&log).unwrap();
        assert!(!calls.lines().any(|line| line == "tmux" || line == "herdr"));
        if fail != "attach" && fail != "do" {
            assert!(
                !calls
                    .lines()
                    .any(|line| line == "attach" || line == "session.select")
            );
        }
    }

    fs::remove_dir_all(root).unwrap();
}
