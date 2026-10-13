//! Loads page previews on a worker thread, so the window never waits for a file.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use image::RgbImage;
use pagecrop_core::decode::{load_oriented, shrink_to_long_edge};
use pagecrop_core::error::CoreError;

/// Longest side of a preview, in pixels.
const PREVIEW_LONG_EDGE: u32 = 1600;
/// Longest side of a thumbnail, in pixels.
const THUMB_LONG_EDGE: u32 = 160;

pub struct PreviewRequest {
    pub index: usize,
    pub generation: u64,
    pub path: PathBuf,
}

pub enum PreviewReply {
    Loaded {
        index: usize,
        generation: u64,
        image: egui::ColorImage,
        /// The same preview as pixels, for rendering the result view.
        rgb: RgbImage,
        full_size: (u32, u32),
    },
    Failed {
        index: usize,
        generation: u64,
        message: String,
    },
}

/// The worker thread and the channels to talk to it.
pub struct Loader {
    requests: Sender<Vec<PreviewRequest>>,
    replies: Receiver<PreviewReply>,
}

impl Loader {
    /// Starts the worker. It wakes the window through `ctx` when a reply is ready.
    pub fn spawn(ctx: egui::Context) -> Loader {
        let (request_tx, request_rx): (Sender<Vec<PreviewRequest>>, _) = mpsc::channel();
        let (reply_tx, reply_rx) = mpsc::channel();
        std::thread::spawn(move || work(&request_rx, &reply_tx, &ctx));
        Loader {
            requests: request_tx,
            replies: reply_rx,
        }
    }

    /// Replaces the wanted previews with `wanted`, in the order they are needed. An empty list
    /// cancels the ones still waiting.
    pub fn request(&self, wanted: Vec<PreviewRequest>) {
        // The worker runs until the window closes, so a failed send is not an error.
        let _ = self.requests.send(wanted);
    }

    /// The next finished preview, if one is waiting.
    pub fn poll(&self) -> Option<PreviewReply> {
        self.replies.try_recv().ok()
    }
}

/// Decodes the wanted previews one at a time. A newer list replaces what is left of the older one.
fn work(
    requests: &Receiver<Vec<PreviewRequest>>,
    replies: &Sender<PreviewReply>,
    ctx: &egui::Context,
) {
    let mut pending: VecDeque<PreviewRequest> = VecDeque::new();
    loop {
        if pending.is_empty() {
            match requests.recv() {
                Ok(wanted) => pending = wanted.into(),
                Err(_) => return,
            }
        }
        while let Ok(newer) = requests.try_recv() {
            pending = newer.into();
        }
        let Some(request) = pending.pop_front() else {
            continue;
        };
        if replies.send(build_reply(&request)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

fn build_reply(request: &PreviewRequest) -> PreviewReply {
    let (index, generation) = (request.index, request.generation);
    match load_preview(&request.path) {
        Ok((image, rgb, full_size)) => PreviewReply::Loaded {
            index,
            generation,
            image,
            rgb,
            full_size,
        },
        Err(err) => PreviewReply::Failed {
            index,
            generation,
            message: err.to_string(),
        },
    }
}

/// The preview picture, ready to upload, and the size of the whole picture before shrinking.
fn load_preview(path: &Path) -> Result<(egui::ColorImage, RgbImage, (u32, u32)), CoreError> {
    let full = load_oriented(path)?;
    let full_size = full.dimensions();
    let preview = shrink_to_long_edge(&full, PREVIEW_LONG_EDGE);
    let size = [preview.width() as usize, preview.height() as usize];
    let color = egui::ColorImage::from_rgb(size, preview.as_raw());
    Ok((color, preview, full_size))
}

/// A finished thumbnail. `image` is None when the picture could not be decoded.
/// The file name identifies the page, because a sort can move the page to another index.
pub struct Thumb {
    pub file_name: String,
    pub generation: u64,
    pub image: Option<egui::ColorImage>,
}

/// Makes thumbnails on a pool of its own, so that they never hold up the preview.
pub struct Thumbnails {
    pool: rayon::ThreadPool,
    ctx: egui::Context,
    replies_tx: Sender<Thumb>,
    replies: Receiver<Thumb>,
}

impl Thumbnails {
    /// One thread per core but one, and at least one.
    pub fn new(ctx: egui::Context) -> Result<Thumbnails, rayon::ThreadPoolBuildError> {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cores.saturating_sub(1).max(1))
            .build()?;
        let (replies_tx, replies) = mpsc::channel();
        Ok(Thumbnails {
            pool,
            ctx,
            replies_tx,
            replies,
        })
    }

    pub fn request(&self, generation: u64, file_name: String, path: PathBuf) {
        let tx = self.replies_tx.clone();
        let ctx = self.ctx.clone();
        self.pool.spawn(move || {
            let image = make_thumbnail(&path);
            let _ = tx.send(Thumb {
                file_name,
                generation,
                image,
            });
            ctx.request_repaint();
        });
    }

    /// The next finished thumbnail, if one is waiting.
    pub fn poll(&self) -> Option<Thumb> {
        self.replies.try_recv().ok()
    }
}

fn make_thumbnail(path: &Path) -> Option<egui::ColorImage> {
    let full = load_oriented(path).ok()?;
    let thumb = shrink_to_long_edge(&full, THUMB_LONG_EDGE);
    let size = [thumb.width() as usize, thumb.height() as usize];
    Some(egui::ColorImage::from_rgb(size, thumb.as_raw()))
}
