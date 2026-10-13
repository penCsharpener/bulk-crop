//! Exports the pictures: each page is rendered with its settings and written to the output folder,
//! several at a time.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::decode::load_oriented;
use crate::encode::write_jpeg;
use crate::error::CoreError;
use crate::geometry::Resize;
use crate::model::{Adjust, Project};
use crate::render::render;

/// Where and how the pictures are exported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportOptions {
    pub out_dir: PathBuf,
    pub resize: Resize,
    /// JPEG quality, 1 to 100.
    pub quality: u8,
    /// Whether an existing file in the output folder is replaced.
    pub overwrite: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            out_dir: PathBuf::new(),
            resize: Resize::LongEdge(1600),
            quality: 90,
            overwrite: false,
        }
    }
}

impl ExportOptions {
    /// The default options for a photo folder: the output goes to a `cropped` folder inside it.
    pub fn for_folder(dir: &Path) -> ExportOptions {
        ExportOptions {
            out_dir: dir.join("cropped"),
            ..ExportOptions::default()
        }
    }
}

/// One picture to export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportJob {
    /// The page's index in the project.
    pub index: usize,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub adjust: Adjust,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Written,
    Skipped,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Started {
        total: usize,
    },
    Item {
        index: usize,
        outcome: Outcome,
    },
    Finished {
        written: usize,
        skipped: usize,
        failed: usize,
        cancelled: bool,
    },
}

/// One job per picture, in display order. Creates the output folder, and refuses the photo folder
/// itself as the output.
pub fn plan(project: &Project, opts: &ExportOptions) -> Result<Vec<ExportJob>, CoreError> {
    std::fs::create_dir_all(&opts.out_dir).map_err(|source| CoreError::Io {
        path: opts.out_dir.clone(),
        source,
    })?;
    if canonical(&opts.out_dir)? == canonical(&project.dir)? {
        return Err(CoreError::OutDirIsSource {
            path: opts.out_dir.clone(),
        });
    }
    Ok(project
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| ExportJob {
            index,
            src: entry.path.clone(),
            dst: opts.out_dir.join(&entry.file_name),
            adjust: project.effective(index),
        })
        .collect())
}

fn canonical(path: &Path) -> Result<PathBuf, CoreError> {
    path.canonicalize().map_err(|source| CoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Exports one picture. An existing file is skipped unless overwriting is on.
pub fn export_one(job: &ExportJob, opts: &ExportOptions) -> Outcome {
    if job.dst.exists() && !opts.overwrite {
        return Outcome::Skipped;
    }
    match write_picture(job, opts) {
        Ok(()) => Outcome::Written,
        Err(err) => Outcome::Failed(err.to_string()),
    }
}

fn write_picture(job: &ExportJob, opts: &ExportOptions) -> Result<(), CoreError> {
    let src = load_oriented(&job.src)?;
    let img = render(&src, &job.adjust, opts.resize);
    write_jpeg(&job.dst, &img, opts.quality)
}

/// Exports every job, on at most eight threads. A job is not started once `cancel` is set.
/// Progress is reported as each picture finishes.
pub fn run(
    jobs: &[ExportJob],
    opts: &ExportOptions,
    cancel: &AtomicBool,
    on_progress: &(dyn Fn(Progress) + Sync),
) -> Result<(), CoreError> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cores.min(8))
        .build()
        .map_err(|err| CoreError::Threads(err.to_string()))?;
    on_progress(Progress::Started { total: jobs.len() });
    let outcomes: Vec<Option<Outcome>> = pool.install(|| {
        jobs.par_iter()
            .map(|job| {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                let outcome = export_one(job, opts);
                on_progress(Progress::Item {
                    index: job.index,
                    outcome: outcome.clone(),
                });
                Some(outcome)
            })
            .collect()
    });
    let (mut written, mut skipped, mut failed) = (0, 0, 0);
    for outcome in outcomes.iter().flatten() {
        match outcome {
            Outcome::Written => written += 1,
            Outcome::Skipped => skipped += 1,
            Outcome::Failed(_) => failed += 1,
        }
    }
    let cancelled = outcomes.iter().any(|o| o.is_none());
    on_progress(Progress::Finished {
        written,
        skipped,
        failed,
        cancelled,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CropRect;
    use pagecrop_testkit::write_book;

    fn options(out_dir: PathBuf, resize: Resize, overwrite: bool) -> ExportOptions {
        ExportOptions {
            out_dir,
            resize,
            quality: 90,
            overwrite,
        }
    }

    /// Six 400x300 pages. Page 1 has default settings; page 4 keeps the left half of the picture,
    /// so pages 4 to 6 follow it.
    fn six_pages(dir: &Path) -> Project {
        write_book(dir, 6, 400, 300, 0.0, false).unwrap();
        let mut project = Project::open(dir).unwrap();
        project.set_key(0, Adjust::default());
        project.set_key(
            3,
            Adjust {
                crop: CropRect {
                    cx: 0.0,
                    cy: 0.0,
                    w: 0.5,
                    h: 1.0,
                },
                ..Adjust::default()
            },
        );
        project
    }

    fn export_all(project: &Project, opts: &ExportOptions) {
        let jobs = plan(project, opts).unwrap();
        run(&jobs, opts, &AtomicBool::new(false), &|_| {}).unwrap();
    }

    #[test]
    fn writes_six_files_with_expected_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let out = dir.path().join("out");
        export_all(
            &project,
            &options(out.clone(), Resize::LongEdge(200), false),
        );
        for page in 1..=3 {
            let path = out.join(format!("page_{page:03}.jpg"));
            assert_eq!(
                image::image_dimensions(path).unwrap(),
                (200, 150),
                "page {page}"
            );
        }
        for page in 4..=6 {
            let path = out.join(format!("page_{page:03}.jpg"));
            assert_eq!(
                image::image_dimensions(path).unwrap(),
                (133, 200),
                "page {page}"
            );
        }
    }

    #[test]
    fn out_dir_equal_to_source_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let opts = options(dir.path().to_path_buf(), Resize::None, false);
        let err = plan(&project, &opts).unwrap_err();
        assert!(matches!(err, CoreError::OutDirIsSource { .. }));
    }

    #[test]
    fn existing_file_is_skipped_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let opts = options(dir.path().join("out"), Resize::LongEdge(200), false);
        export_all(&project, &opts);
        let jobs = plan(&project, &opts).unwrap();
        assert_eq!(export_one(&jobs[0], &opts), Outcome::Skipped);
    }

    #[test]
    fn existing_file_is_replaced_with_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let out = dir.path().join("out");
        let opts = options(out.clone(), Resize::LongEdge(200), true);
        let jobs = plan(&project, &opts).unwrap();
        std::fs::write(out.join("page_001.jpg"), b"old").unwrap();
        assert_eq!(export_one(&jobs[0], &opts), Outcome::Written);
        assert_eq!(
            image::image_dimensions(out.join("page_001.jpg")).unwrap(),
            (200, 150)
        );
    }

    #[test]
    fn cancel_before_start_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let out = dir.path().join("out");
        let opts = options(out.clone(), Resize::LongEdge(200), false);
        let jobs = plan(&project, &opts).unwrap();
        run(&jobs, &opts, &AtomicBool::new(true), &|_| {}).unwrap();
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    }

    #[test]
    fn broken_file_fails_and_others_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        std::fs::write(dir.path().join("page_002.jpg"), b"not a jpeg").unwrap();
        let opts = options(dir.path().join("out"), Resize::LongEdge(200), false);
        let jobs = plan(&project, &opts).unwrap();
        for job in &jobs {
            let outcome = export_one(job, &opts);
            if job.index == 1 {
                assert!(matches!(outcome, Outcome::Failed(_)), "page 2: {outcome:?}");
            } else {
                assert_eq!(outcome, Outcome::Written, "index {}", job.index);
            }
        }
    }

    #[test]
    fn no_tmp_files_remain() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let out = dir.path().join("out");
        export_all(
            &project,
            &options(out.clone(), Resize::LongEdge(200), false),
        );
        let names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 6);
        assert!(names.iter().all(|n| !n.ends_with(".tmp")));
    }

    #[test]
    fn source_files_are_byte_identical_afterwards() {
        let dir = tempfile::tempdir().unwrap();
        let project = six_pages(dir.path());
        let before: Vec<Vec<u8>> = project
            .entries
            .iter()
            .map(|e| std::fs::read(&e.path).unwrap())
            .collect();
        export_all(
            &project,
            &options(dir.path().join("out"), Resize::LongEdge(200), false),
        );
        for (entry, bytes) in project.entries.iter().zip(before) {
            assert_eq!(
                std::fs::read(&entry.path).unwrap(),
                bytes,
                "{}",
                entry.file_name
            );
        }
    }
}
