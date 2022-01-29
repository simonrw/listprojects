use std::io::Read;

use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

use crate::Selectable;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct Cache(Vec<Selectable>);

impl Cache {
    fn from_reader(r: impl Read) -> Result<Self> {
        let cache = serde_json::from_reader(r).wrap_err("reading cache")?;
        Ok(cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Selectable;

    use std::{
        io::{prelude::*, SeekFrom},
        path::PathBuf,
    };

    #[test]
    fn load_empty() {
        temp_file_with_contents("[]", |file| {
            let cache = Cache::from_reader(file).unwrap();

            assert_eq!(cache, Cache(Vec::new()));
            Ok(())
        });
    }

    #[test]
    fn load_blank() {
        temp_file_with_contents(r#"[{"path": "", "short_name": ""}]"#, |file| {
            let cache = Cache::from_reader(file).unwrap();

            assert_eq!(
                cache,
                Cache(vec![Selectable {
                    path: PathBuf::from(""),
                    short_name: "".to_string(),
                    prefix: None,
                }])
            );
            Ok(())
        });
    }

    #[test]
    fn load_with_values() {
        temp_file_with_contents(
            r#"[{"path": "/a/b/c", "short_name": "something", "prefix": "a/"}]"#,
            |file| {
                let cache = Cache::from_reader(file).unwrap();

                assert_eq!(
                    cache,
                    Cache(vec![Selectable {
                        path: PathBuf::from("/a/b/c"),
                        short_name: "something".to_string(),
                        prefix: Some("a/".to_string()),
                    }])
                );
                Ok(())
            },
        );
    }

    // helper functions
    fn temp_file_with_contents<F>(contents: &str, cb: F)
    where
        F: Fn(&mut std::fs::File) -> Result<()>,
    {
        let mut file = tempfile::tempfile().unwrap();
        write!(&mut file, "{}", contents).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        cb(&mut file).unwrap()
    }
}
