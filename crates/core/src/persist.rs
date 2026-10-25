//! The settings file of this version of pagecrop, kept in the photo folder: the sort and the settings
//! of each page that has its own. Each minor version has its own file and there is no migration.

use std::collections::BTreeMap;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CoreError;
use crate::export::ExportOptions;
use crate::model::{Adjust, Project, SortBy};

/// The settings file name, kept in the photo folder. It carries the major and minor version of the
/// tool, so `pagecrop_v05.json` belongs to 0.5.x.
pub const SIDECAR_NAME: &str = concat!(
    "pagecrop_v",
    env!("CARGO_PKG_VERSION_MAJOR"),
    env!("CARGO_PKG_VERSION_MINOR"),
    ".json"
);

/// What `load` finds: the sort and the settings of each page that has its own, by file name.
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub sort: SortBy,
    pub keys: BTreeMap<String, Adjust>,
    /// The export options, if the file has them.
    pub export: Option<ExportOptions>,
    /// The parts of the file that could not be read and were left out: a page's file name for a key,
    /// or `sort`, `keys` or `export`.
    pub dropped: Vec<String>,
}

/// The settings file as it is written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct File {
    sort: SortBy,
    keys: BTreeMap<String, Adjust>,
    export: Option<ExportOptions>,
}

impl Default for File {
    fn default() -> Self {
        File {
            sort: SortBy::Name,
            keys: BTreeMap::new(),
            export: None,
        }
    }
}

/// Writes the settings of every page that has its own, and of the orphans, to the settings file.
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

/// The saved settings of a folder, or None when the folder has no settings file of this version.
/// Text that is not a JSON object is an error. Otherwise every part is read on its own: a part
/// that cannot be read is left out and named in `Saved::dropped`, a missing part takes its default
/// and extra fields are ignored.
pub fn load(dir: &Path) -> Result<Option<Saved>, CoreError> {
    let path = dir.join(SIDECAR_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(&path, source)),
    };
    let sidecar_error = |message: String| CoreError::Sidecar {
        path: path.clone(),
        message,
    };
    let value: Value = serde_json::from_str(&text).map_err(|err| sidecar_error(err.to_string()))?;
    let Value::Object(mut object) = value else {
        return Err(sidecar_error("not a JSON object".to_owned()));
    };
    let mut dropped = Vec::new();
    let sort = read_part(object.remove("sort"), "sort", &mut dropped);
    let keys = read_keys(object.remove("keys"), &mut dropped);
    let export = read_part(object.remove("export"), "export", &mut dropped);
    Ok(Some(Saved {
        sort,
        keys,
        export,
        dropped,
    }))
}

/// One part of the file: the default when it is missing, and the default again, with `name` added to
/// `dropped`, when it cannot be read.
fn read_part<T: DeserializeOwned + Default>(
    part: Option<Value>,
    name: &str,
    dropped: &mut Vec<String>,
) -> T {
    let Some(value) = part else {
        return T::default();
    };
    serde_json::from_value(value).unwrap_or_else(|_| {
        dropped.push(name.to_owned());
        T::default()
    })
}

/// The page settings of the file, each entry read on its own. Entries that cannot be read are named
/// in `dropped`; `keys` that is not an object is dropped as `keys`.
fn read_keys(part: Option<Value>, dropped: &mut Vec<String>) -> BTreeMap<String, Adjust> {
    let mut keys = BTreeMap::new();
    match part {
        None => {}
        Some(Value::Object(entries)) => {
            for (name, value) in entries {
                match serde_json::from_value::<Adjust>(value) {
                    Ok(adjust) => {
                        keys.insert(name, adjust);
                    }
                    Err(_) => dropped.push(name),
                }
            }
        }
        Some(_) => dropped.push("keys".to_owned()),
    }
    keys
}

/// The names of the settings files of other versions in `dir`: files whose name starts with
/// `pagecrop` and ends with `.json`, except `SIDECAR_NAME`, sorted. A folder that cannot be read
/// gives an empty list, because this is only a hint.
pub fn older_settings(dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("pagecrop") && name.ends_with(".json"))
        .filter(|name| name != SIDECAR_NAME)
        .collect();
    names.sort();
    names
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

    /// Writes `text` as the settings file of `dir`.
    fn write_settings(dir: &tempfile::TempDir, text: &str) {
        std::fs::write(dir.path().join(SIDECAR_NAME), text).unwrap();
    }

    #[test]
    fn settings_file_name_carries_the_version() {
        assert_eq!(
            SIDECAR_NAME,
            format!(
                "pagecrop_v{}{}.json",
                env!("CARGO_PKG_VERSION_MAJOR"),
                env!("CARGO_PKG_VERSION_MINOR")
            )
        );
        assert_ne!(SIDECAR_NAME, "pagecrop.json");
    }

    #[test]
    fn unreadable_key_is_dropped_and_named() {
        let dir = folder_with(&[]);
        write_settings(
            &dir,
            r#"{"keys":{"page_001.jpg":{"brightness":5},"page_002.jpg":{"brightness":"loud"}}}"#,
        );
        let saved = load(dir.path()).unwrap().unwrap();
        assert_eq!(saved.keys.len(), 1);
        assert_eq!(saved.keys.get("page_001.jpg"), Some(&brightness(5)));
        assert_eq!(saved.dropped, ["page_002.jpg"]);
    }

    #[test]
    fn wrong_sort_falls_back_to_name() {
        let dir = folder_with(&[]);
        write_settings(&dir, r#"{"sort":"sideways","keys":{}}"#);
        let saved = load(dir.path()).unwrap().unwrap();
        assert_eq!(saved.sort, SortBy::Name);
        assert_eq!(saved.dropped, ["sort"]);
    }

    #[test]
    fn wrong_export_is_dropped() {
        let dir = folder_with(&[]);
        write_settings(&dir, r#"{"sort":"date","export":7}"#);
        let saved = load(dir.path()).unwrap().unwrap();
        assert_eq!(saved.export, None);
        assert_eq!(saved.sort, SortBy::Date);
        assert_eq!(saved.dropped, ["export"]);
    }

    #[test]
    fn keys_that_are_not_an_object_drop_as_keys() {
        let dir = folder_with(&[]);
        write_settings(&dir, r#"{"sort":"date","keys":[1,2]}"#);
        let saved = load(dir.path()).unwrap().unwrap();
        assert!(saved.keys.is_empty());
        assert_eq!(saved.sort, SortBy::Date);
        assert_eq!(saved.dropped, ["keys"]);
    }

    #[test]
    fn json_that_is_not_an_object_is_sidecar_error() {
        let dir = folder_with(&[]);
        write_settings(&dir, "[1, 2]");
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, CoreError::Sidecar { .. }));
    }

    #[test]
    fn old_file_without_version_name_is_not_read() {
        let dir = folder_with(&[]);
        std::fs::write(
            dir.path().join("pagecrop.json"),
            r#"{"version":1,"keys":{"page_001.jpg":{"brightness":5}}}"#,
        )
        .unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
    }

    #[test]
    fn older_settings_lists_other_versions() {
        let dir = folder_with(&[
            "pagecrop.json",
            "pagecrop_v04.json",
            SIDECAR_NAME,
            "pagecrop.json.bak",
            "notes.json",
        ]);
        std::fs::create_dir(dir.path().join("pagecrop_x.json")).unwrap();
        assert_eq!(
            older_settings(dir.path()),
            ["pagecrop.json", "pagecrop_v04.json"]
        );
    }

    #[test]
    fn older_settings_of_missing_folder_is_empty() {
        let dir = folder_with(&[]);
        assert!(older_settings(&dir.path().join("nowhere")).is_empty());
    }

    #[test]
    fn file_without_alternate_loads_as_false() {
        let dir = folder_with(&["page_001.jpg"]);
        std::fs::write(
            dir.path().join(SIDECAR_NAME),
            r#"{"version":1,"sort":"name","keys":{"page_001.jpg":{"quarter_turns":1,"angle_deg":2.0,"crop":{"cx":0.0,"cy":0.0,"w":1.0,"h":1.0},"brightness":5,"contrast":0,"grayscale":false}}}"#,
        )
        .unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        let key = saved.keys.get("page_001.jpg").unwrap();
        assert!(!key.alternate);
        assert_eq!(key.brightness, 5);
    }

    #[test]
    fn frame_without_corner_shift_loads_as_rectangle() {
        let dir = folder_with(&["page_001.jpg"]);
        std::fs::write(
            dir.path().join(SIDECAR_NAME),
            r#"{"version":1,"sort":"name","keys":{"page_001.jpg":{"crop":{"cx":0.1,"cy":0.0,"w":0.7,"h":0.8}}}}"#,
        )
        .unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        let crop = saved.keys.get("page_001.jpg").unwrap().crop;
        assert_eq!(crop.corner_shift, [(0.0, 0.0); 4]);
        assert!((crop.w - 0.7).abs() < 1e-4);
    }

    #[test]
    fn corner_shift_round_trips() {
        let dir = folder_with(&["page_001.jpg"]);
        let mut project = Project::open(dir.path()).unwrap();
        let shift = [(0.07, 0.0), (-0.07, 0.0), (0.0, 0.02), (0.0, 0.0)];
        project.set_key(
            0,
            Adjust {
                crop: crate::model::CropRect {
                    w: 0.7,
                    h: 0.8,
                    corner_shift: shift,
                    ..crate::model::CropRect::default()
                },
                ..Adjust::default()
            },
        );
        save(&project).unwrap();
        let saved = load(dir.path()).unwrap().unwrap();
        let key = saved.keys.get("page_001.jpg").unwrap();
        assert_eq!(key.crop.corner_shift, shift);
    }
}
