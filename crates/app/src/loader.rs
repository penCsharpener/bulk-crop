//! Loads page previews on a worker thread, so the window never waits for a file.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use pagecrop_core::decode::{load_oriented, shrink_to_long_edge};
use pagecrop_core::error::CoreError;

/// Longest side of a preview, in pixels.
const PREVIEW_LONG_EDGE: u32 = 1600;

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
    requests: Sender<PreviewRequest>,
    replies: Receiver<PreviewReply>,
}

impl Loader {
    /// Starts the worker. It wakes the window through `ctx` when a reply is ready.
    pub fn spawn(ctx: egui::Context) -> Loader {
        let (request_tx, request_rx) = mpsc::channel();
        let (reply_tx, reply_rx) = mpsc::channel();
        std::thread::spawn(move || work(&request_rx, &reply_tx, &ctx));
        Loader {
            requests: request_tx,
            replies: reply_rx,
        }
    }

    pub fn request(&self, request: PreviewRequest) {
        // The worker runs until the window closes, so a failed send is not an error.
        let _ = self.requests.send(request);
    }

    /// The next finished preview, if one is waiting.
    pub fn poll(&self) -> Option<PreviewReply> {
        self.replies.try_recv().ok()
    }
}

fn work(requests: &Receiver<PreviewRequest>, replies: &Sender<PreviewReply>, ctx: &egui::Context) {
    while let Ok(mut request) = requests.recv() {
        // Skip to the newest request that is already waiting.
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }
        if replies.send(build_reply(&request)).is_err() {
            break;
        }
        ctx.request_repaint();
    }
}

fn build_reply(request: &PreviewRequest) -> PreviewReply {
    let (index, generation) = (request.index, request.generation);
    match load_preview(&request.path) {
        Ok((image, full_size)) => PreviewReply::Loaded {
            index,
            generation,
            image,
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
fn load_preview(path: &Path) -> Result<(egui::ColorImage, (u32, u32)), CoreError> {
    let full = load_oriented(path)?;
    let full_size = full.dimensions();
    let preview = shrink_to_long_edge(&full, PREVIEW_LONG_EDGE);
    let size = [preview.width() as usize, preview.height() as usize];
    Ok((
        egui::ColorImage::from_rgb(size, preview.as_raw()),
        full_size,
    ))
}
