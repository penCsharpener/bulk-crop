//! The settings file `pagecrop.json`, kept in the photo folder: the sort and the settings of each
//! page that has its own.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::export::ExportOptions;
use crate::model::{Adjust, Project, SortBy};

/// The settings file name, kept in the photo folder.
pub const SIDECAR_NAME: &str = "pagecrop.json";

/// What `load` finds: the sort and the settings of each page that has its own, by file name.
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub sort: SortBy,
    pub keys: BTreeMap<String, Adjust>,
    /// The export options, if the file has them.
    pub export: Option<ExportOptions>,
}

/// The settings file as it is written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct File {
    version: u32,
    sort: SortBy,
    keys: BTreeMap<String, Adjust>,
    export: Option<ExportOptions>,
}

impl Default for File {
    fn default() -> Self {
        File {
            version: 1,
            sort: SortBy::Name,
            keys: BTreeMap::new(),
            export: None,
        }
    }
}

/// Writes the settings of every page that has its own, and of the orphans, to `pagecrop.json`.
/// The file is written under a temporary name first and then renamed.
pub fn save(project: &Project) -> Result<(), CoreError> {
    let keys: BTreeMap<String, Adjust> = project
        .entries
        .iter()
        .filter_map(|e| e.key.map(|k| (e.file_name.clone(), k)))
        .chain(
            project
                .orphans
                .iter()
                .map(|(name, adjust)| (name.clone(), *adjust)),
        )
        .collect();
    let file = File {
        sort: project.sort,
        keys,
        export: Some(project.export.clone()),
        ..File::default()
    };
    let path = project.dir.join(SIDECAR_NAME);
    let tmp = project.dir.join(format!("{SIDECAR_NAME}.tmp"));
    let text = serde_json::to_string_pretty(&file).map_err(|err| CoreError::Sidecar {
        path: path.clone(),
        message: err.to_string(),
    })?;
    std::fs::write(&tmp, text).map_err(|source| io_error(&tmp, source))?;
    std::fs::rename(&tmp, &path).map_err(|source| {
        let _ = std::fs::remove_file(&tmp);
        io_error(&path, source)
    })
}

/// The saved settings of a folder, or None when the folder has no settings file. A file that
/// cannot be parsed is an error; extra fields are ignored.
pub fn load(dir: &Path) -> Result<Option<Saved>, CoreError> {
    let path = dir.join(SIDECAR_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(&path, source)),
    };
    let file: File = serde_json::from_str(&text).map_err(|err| CoreError::Sidecar {
        path: path.clone(),
        message: err.to_string(),
    })?;
    Ok(Some(Saved {
        sort: file.sort,
        keys: file.keys,
        export: file.export,
    }))
}

fn io_error(path: &Path, source: std::io::Error) -> CoreError {
    CoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Resize;

    fn folder_with(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in names {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        dir
    }

    fn brightness(value: i32) -> Adjust {
        Adjust {
            brightness: value,
            ..Adjust::default()
        }
    }

    #[test]
    fn save_then_load_round_trips_keys_and_sort() {
        let dir = folder_with(&["page_001.jpg", "page_002.jpg"]);
        let mut project = Project::open(dir.path()).unwrap();
        project.set_key(1, brightness(5));
        project.sort(SortBy::Date);
        save(&project).unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        assert_eq!(saved.sort, SortBy::Date);
        assert_eq!(saved.keys.get("page_002.jpg"), Some(&brightness(5)));
        assert_eq!(saved.keys.len(), 1);
    }

    #[test]
    fn key_for_missing_file_becomes_orphan_and_is_saved_again() {
        let dir = folder_with(&["page_001.jpg"]);
        let mut keys = BTreeMap::new();
        keys.insert("gone.jpg".to_owned(), brightness(9));
        let file = File {
            keys,
            ..File::default()
        };
        std::fs::write(
            dir.path().join(SIDECAR_NAME),
            serde_json::to_string(&file).unwrap(),
        )
        .unwrap();
        let project = Project::open(dir.path()).unwrap();
        assert_eq!(project.orphans.get("gone.jpg"), Some(&brightness(9)));
        save(&project).unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        assert_eq!(saved.keys.get("gone.jpg"), Some(&brightness(9)));
    }

    #[test]
    fn absent_file_is_none() {
        let dir = folder_with(&[]);
        assert_eq!(load(dir.path()).unwrap(), None);
    }

    #[test]
    fn garbage_file_is_sidecar_error() {
        let dir = folder_with(&[]);
        std::fs::write(dir.path().join(SIDECAR_NAME), "{").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, CoreError::Sidecar { .. }));
        assert!(err.to_string().contains(SIDECAR_NAME));
    }

    #[test]
    fn no_tmp_file_left_after_save() {
        let dir = folder_with(&["page_001.jpg"]);
        let project = Project::open(dir.path()).unwrap();
        save(&project).unwrap();
        assert!(dir.path().join(SIDECAR_NAME).is_file());
        assert!(!dir.path().join(format!("{SIDECAR_NAME}.tmp")).exists());
    }

    #[test]
    fn file_with_unknown_extra_field_still_loads() {
        let dir = folder_with(&[]);
        std::fs::write(
            dir.path().join(SIDECAR_NAME),
            r#"{"version":1,"sort":"name","keys":{},"extra":42}"#,
        )
        .unwrap();
        assert!(load(dir.path()).unwrap().is_some());
    }

    #[test]
    fn export_options_round_trip() {
        let dir = folder_with(&["page_001.jpg"]);
        let mut project = Project::open(dir.path()).unwrap();
        project.export.quality = 70;
        project.export.overwrite = true;
        project.export.resize = Resize::Percent(40.0);
        save(&project).unwrap();
        let reopened = Project::open(dir.path()).unwrap();
        assert_eq!(reopened.export, project.export);
    }

    #[test]
    fn file_without_export_gets_the_folder_default() {
        let dir = folder_with(&["page_001.jpg"]);
        std::fs::write(
            dir.path().join(SIDECAR_NAME),
            r#"{"version":1,"sort":"name","keys":{}}"#,
        )
        .unwrap();
        assert_eq!(load(dir.path()).unwrap().unwrap().export, None);
        let project = Project::open(dir.path()).unwrap();
        assert_eq!(project.export, ExportOptions::for_folder(dir.path()));
    }
}
