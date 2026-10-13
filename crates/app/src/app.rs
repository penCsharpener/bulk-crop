//! The application state and the frame that lays out the panels.

use crate::{editor, panels};

/// All state of the window. Empty until later rows add fields.
#[derive(Default)]
pub struct App {}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        panels::top::show(ui);
        panels::status::show(ui);
        panels::pages::show(ui);
        panels::adjust::show(ui);
        editor::show(ui);
    }
}
