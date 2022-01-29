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
    use crate::Selectable;

    use super::Cache;
    use std::{
        io::{prelude::*, SeekFrom},
        path::PathBuf,
    };

    #[test]
    fn load_empty() {
        let mut file = tempfile::tempfile().unwrap();
        write!(&mut file, "[]").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let cache = Cache::from_reader(file).unwrap();

        assert_eq!(cache, Cache(Vec::new()));
    }

    #[test]
    fn load_blank() {
        let mut file = tempfile::tempfile().unwrap();
        write!(&mut file, r#"[{{"path": "", "short_name": ""}}]"#).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let cache = Cache::from_reader(file).unwrap();

        assert_eq!(
            cache,
            Cache(vec![Selectable {
                path: PathBuf::from(""),
                short_name: "".to_string(),
                prefix: None,
            }])
        );
    }
}
