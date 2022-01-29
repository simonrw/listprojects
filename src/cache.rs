use std::{io::Read, path::Path};

use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

use crate::Selectable;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq)]
pub(crate) struct Cache(Vec<Selectable>);

impl Cache {
    fn from_reader(r: impl Read) -> Result<Self> {
        let cache = serde_json::from_reader(r).wrap_err("reading cache")?;
        Ok(cache)
    }

    fn open(p: impl AsRef<Path>) -> Result<Self> {
        let p = p.as_ref();
        let cache = std::fs::File::open(p)
            .map_err(From::from)
            .and_then(|f| Self::from_reader(f))
            .unwrap_or_default();
        Ok(cache)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
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

    #[test]
    fn load_with_no_cache_present() {
        let tdir = tempfile::tempdir().unwrap();
        let filename = tdir.path().join("config.json");

        let cache = Cache::open(&filename).unwrap();

        assert!(cache.is_empty());
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
