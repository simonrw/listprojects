use std::{collections::HashSet, io::Read, io::Write, path::Path};

use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

use crate::Selectable;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq)]
pub(crate) struct Cache(HashSet<Selectable>);

impl Cache {
    pub(crate) fn from_reader(r: impl Read) -> Result<Self> {
        let cache = serde_json::from_reader(r).wrap_err("reading cache")?;
        Ok(cache)
    }

    pub(crate) fn open(p: impl AsRef<Path>) -> Result<Self> {
        let p = p.as_ref();
        let cache = std::fs::File::open(p)
            .map_err(From::from)
            .and_then(Self::from_reader)
            .unwrap_or_default();
        Ok(cache)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn write_to(&self, w: impl Write) -> Result<()> {
        serde_json::to_writer(w, self).wrap_err("writing cache")?;
        Ok(())
    }

    pub(crate) fn save(&self, filename: impl AsRef<Path>) -> Result<()> {
        let mut f = std::fs::File::create(filename).wrap_err("creating output file")?;
        self.write_to(&mut f).wrap_err("writing to output file")?;
        Ok(())
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

            assert_eq!(cache, Cache(HashSet::new()));
            Ok(())
        });
    }

    #[test]
    fn load_blank() {
        temp_file_with_contents(r#"[{"path": "", "short_name": ""}]"#, |file| {
            let cache = Cache::from_reader(file).unwrap();

            let expected = Cache(HashSet::from_iter(vec![Selectable {
                path: PathBuf::from(""),
                short_name: "".to_string(),
                prefix: None,
            }]));
            assert_eq!(cache, expected);
            Ok(())
        });
    }

    #[test]
    fn load_with_values() {
        temp_file_with_contents(
            r#"[{"path": "/a/b/c", "short_name": "something", "prefix": "a/"}]"#,
            |file| {
                let cache = Cache::from_reader(file).unwrap();

                let expected = Cache(HashSet::from_iter(vec![Selectable {
                    path: PathBuf::from("/a/b/c"),
                    short_name: "something".to_string(),
                    prefix: Some("a/".to_string()),
                }]));
                assert_eq!(cache, expected);
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

    #[test]
    fn saving() {
        let cache = Cache(HashSet::from_iter(vec![Selectable {
            path: PathBuf::from("/a/b/c"),
            short_name: "short-name".to_string(),
            prefix: None,
        }]));
        let mut f = tempfile::NamedTempFile::new().unwrap();
        cache.write_to(&mut f).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();

        let read_cache = Cache::from_reader(f).unwrap();
        assert_eq!(read_cache, cache);
    }

    #[test]
    fn overwriting() {
        let cache = Cache(HashSet::from_iter(vec![Selectable {
            path: PathBuf::from("/a/b/c"),
            short_name: "short-name".to_string(),
            prefix: None,
        }]));
        let tdir = tempfile::tempdir().unwrap();
        let tpath = tdir.path().join("cache.json");
        dbg!(&tpath);
        let mut f = std::fs::File::create(&tpath).unwrap();
        write!(&mut f, "contents").unwrap();
        f.flush().unwrap();
        drop(f);

        cache.save(&tpath).unwrap();

        let f2 = std::fs::File::open(&tpath).unwrap();
        let read_cache = Cache::from_reader(f2).unwrap();
        assert_eq!(read_cache, cache);
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
