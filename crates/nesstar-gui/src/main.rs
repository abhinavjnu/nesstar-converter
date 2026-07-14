use std::{
    env,
    process::{Child, Command},
    time::Duration,
};

use eframe::egui;
use nesstar_core::pipeline::OutputFormat;

const ALL_FORMATS: &[OutputFormat] = &[
    OutputFormat::Csv,
    OutputFormat::Tsv,
    OutputFormat::Parquet,
    OutputFormat::Dta,
];

fn main() -> eframe::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("--worker") {
        return worker(&args[1..]);
    }
    eframe::run_native(
        "Nesstar Converter",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::<ConverterApp>::default())),
    )
}

fn worker(args: &[String]) -> eframe::Result<()> {
    // args: <input.Nesstar> <ddi.xml> <output_path>
    if args.len() != 3 {
        eprintln!("Worker usage: --worker <input.Nesstar> <ddi.xml> <output>");
        std::process::exit(2);
    }
    match nesstar_core::pipeline::convert(&args[0], &args[1], &args[2], 10_000, || true) {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("Conversion failed: {error}");
            std::process::exit(1);
        }
    }
}

struct ConverterApp {
    input: String,
    ddi: String,
    output: String,
    format: OutputFormat,
    worker: Option<Child>,
    status: String,
}

impl Default for ConverterApp {
    fn default() -> Self {
        Self {
            input: String::new(),
            ddi: String::new(),
            output: String::new(),
            format: OutputFormat::Csv,
            worker: None,
            status: String::new(),
        }
    }
}

impl ConverterApp {
    /// Ensure the output path extension matches the selected format.
    fn sync_output_extension(&mut self) {
        if self.output.is_empty() {
            return;
        }
        let path = std::path::Path::new(&self.output);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let dir = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let new_name = format!("{stem}.{}", self.format.extension());
        self.output = if dir.is_empty() {
            new_name
        } else {
            format!("{dir}/{new_name}")
        };
    }

    fn start(&mut self) {
        if self.input.is_empty() || self.ddi.is_empty() || self.output.is_empty() {
            self.status =
                "Choose a Nesstar file, its DDI XML, and an output path.".into();
            return;
        }
        match env::current_exe().and_then(|exe| {
            Command::new(exe)
                .args(["--worker", &self.input, &self.ddi, &self.output])
                .spawn()
        }) {
            Ok(child) => {
                self.worker = Some(child);
                self.status = format!(
                    "Converting to {} …",
                    self.format.label()
                );
            }
            Err(error) => {
                self.status = format!("Could not start conversion worker: {error}")
            }
        }
    }

    fn poll_worker(&mut self) {
        let Some(worker) = self.worker.as_mut() else {
            return;
        };
        match worker.try_wait() {
            Ok(Some(status)) => {
                self.worker = None;
                self.status = if status.success() {
                    format!(
                        "Conversion complete. {} file is ready.",
                        self.format.label()
                    )
                } else {
                    "Conversion failed. Check the source and DDI files.".into()
                };
            }
            Ok(None) => {}
            Err(error) => {
                self.worker = None;
                self.status = format!("Could not read worker status: {error}");
            }
        }
    }
}

impl eframe::App for ConverterApp {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        self.poll_worker();
        if self.worker.is_some() {
            context.request_repaint_after(Duration::from_millis(200));
        }

        egui::CentralPanel::default().show(context, |ui| {
            ui.heading("Nesstar Converter");
            ui.label("Convert a Nesstar survey and its DDI metadata to your chosen format.");
            ui.add_space(12.0);

            // ── Input files ──────────────────────────────────────────────
            file_row(ui, "Nesstar file", &mut self.input, "Choose Nesstar file", false);
            file_row(ui, "DDI XML     ", &mut self.ddi, "Choose DDI XML", false);
            ui.add_space(8.0);

            // ── Format selector ──────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Output format");
                let prev = self.format;
                egui::ComboBox::from_id_salt("format_picker")
                    .selected_text(self.format.label())
                    .show_ui(ui, |ui| {
                        for &fmt in ALL_FORMATS {
                            ui.selectable_value(&mut self.format, fmt, fmt.label());
                        }
                    });
                if self.format != prev {
                    self.sync_output_extension();
                }
            });
            ui.add_space(4.0);

            // ── Output path ──────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Output file ");
                ui.text_edit_singleline(&mut self.output);
                if ui.button("Save as…").clicked() {
                    let ext = self.format.extension();
                    let dialog = rfd::FileDialog::new()
                        .add_filter(self.format.label(), &[ext]);
                    if let Some(path) = dialog.save_file() {
                        self.output = path.display().to_string();
                        // Ensure the extension is correct
                        if !self.output.ends_with(&format!(".{ext}")) {
                            self.output.push('.');
                            self.output.push_str(ext);
                        }
                    }
                }
            });

            ui.add_space(12.0);

            // ── Convert button ───────────────────────────────────────────
            let busy = self.worker.is_some();
            if ui
                .add_enabled(!busy, egui::Button::new(format!("Convert to {}", self.format.label())))
                .clicked()
            {
                self.start();
            }
            if busy {
                ui.spinner();
            }
            if !self.status.is_empty() {
                ui.add_space(8.0);
                ui.label(&self.status);
            }
        });
    }
}

fn file_row(ui: &mut egui::Ui, label: &str, value: &mut String, button: &str, save: bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
        if ui.button(button).clicked() {
            let dialog = rfd::FileDialog::new();
            let selected = if save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(path) = selected {
                *value = path.display().to_string();
            }
        }
    });
}
