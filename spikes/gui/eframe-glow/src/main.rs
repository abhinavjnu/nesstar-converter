use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;

const INITIAL_FILES: [&str; 2] = [
    "/Users/researcher/Survey-2024.Nesstar",
    "/Users/researcher/Household-2024.Nesstar",
];

fn main() -> eframe::Result<()> {
    eframe::run_native(
        concat!("Nesstar Converter — ", env!("CARGO_PKG_NAME")),
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 720.0]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::<GuiSpike>::default())),
    )
}

struct GuiSpike {
    files: Vec<PathBuf>,
    formats: [bool; 8],
    output_directory: Option<PathBuf>,
    progress: f32,
    log: Vec<String>,
    results: Vec<String>,
}

impl Default for GuiSpike {
    fn default() -> Self {
        Self {
            files: INITIAL_FILES.iter().map(PathBuf::from).collect(),
            formats: [true, true, false, true, false, false, false, false],
            output_directory: None,
            progress: 0.42,
            log: vec!["Queued two representative survey files.".into(), "Renderer spike: conversion is intentionally disabled.".into()],
            results: vec!["Example result: Survey-2024 — ready for conversion".into()],
        }
    }
}

impl GuiSpike {
    fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("nesstar")) && !self.files.contains(&path) {
                self.log.push(format!("Queued {}", path.display()));
                self.files.push(path);
            }
        }
    }

    fn choose_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Nesstar files", &["Nesstar", "nesstar"])
            .pick_files()
        {
            self.add_paths(paths);
        }
    }

    fn choose_output_directory(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.log.push(format!("Output folder selected: {}", path.display()));
            self.output_directory = Some(path);
        }
    }
}

impl eframe::App for GuiSpike {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        self.add_paths(dropped.into_iter().filter_map(|file| file.path));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Nesstar Converter");
            ui.label("GUI renderer spike — no survey data is read or converted.");
            ui.separator();

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_height(104.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Drop .Nesstar files here").size(18.0));
                    ui.label("or choose files with the native file dialog");
                    if ui.button("Browse for .Nesstar files…").clicked() { self.choose_files(); }
                });
            });

            ui.add_space(8.0);
            ui.label(format!("Queued files ({})", self.files.len()));
            egui::ScrollArea::vertical().max_height(105.0).show(ui, |ui| {
                let mut remove = None;
                for (index, file) in self.files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(file.file_name().and_then(|n| n.to_str()).unwrap_or("Unnamed file"));
                        ui.small("Companion ddi.xml: example status");
                        if ui.button(format!("Remove file {}", index + 1)).clicked() { remove = Some(index); }
                    });
                }
                if let Some(index) = remove { self.files.remove(index); }
            });

            ui.separator();
            ui.label("Output formats");
            egui::Grid::new("formats").num_columns(4).show(ui, |ui| {
                for (index, label) in ["CSV (.csv)", "Excel (.xlsx)", "Stata (.dta)", "Parquet (.parquet)", "JSON (.json)", "JSON Lines (.jsonl)", "TSV (.tsv)", "Fixed-width (.txt)"].iter().enumerate() {
                    ui.checkbox(&mut self.formats[index], *label);
                    if index % 4 == 3 { ui.end_row(); }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Output folder:");
                let output_text = self.output_directory.as_ref().map_or_else(
                    || "Same folder as source files".to_owned(),
                    |path| path.display().to_string(),
                );
                ui.monospace(output_text);
                if ui.button("Choose output folder…").clicked() { self.choose_output_directory(); }
            });

            ui.separator();
            ui.label(format!("Progress: {:.0}%", self.progress * 100.0));
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());
            if ui.button("Abort conversion").clicked() { self.log.push("Abort requested (spike only; no worker exists).".into()); }

            ui.label("Activity log");
            egui::ScrollArea::vertical().max_height(90.0).show(ui, |ui| { for line in &self.log { ui.monospace(line); } });
            ui.label("Results");
            for result in &self.results { ui.colored_label(Color32::DARK_GREEN, result); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_nesstar_paths_only_once() {
        let mut app = GuiSpike::default();
        let original = app.files.len();
        app.add_paths([PathBuf::from("/tmp/example.Nesstar"), PathBuf::from("/tmp/example.Nesstar"), PathBuf::from("/tmp/not-a-survey.txt")]);
        assert_eq!(app.files.len(), original + 1);
    }
}
