use std::{
    env,
    process::{Child, Command},
    time::Duration,
};

use eframe::egui;

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
    if args.len() != 3 {
        eprintln!("Worker usage: --worker <input.Nesstar> <ddi.xml> <output.csv>");
        std::process::exit(2);
    }
    match nesstar_core::pipeline::convert_csv(&args[0], &args[1], &args[2], 10_000, || true) {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("Conversion failed: {error}");
            std::process::exit(1);
        }
    }
}

#[derive(Default)]
struct ConverterApp {
    input: String,
    ddi: String,
    output: String,
    worker: Option<Child>,
    status: String,
}

impl ConverterApp {
    fn start(&mut self) {
        if self.input.is_empty() || self.ddi.is_empty() || self.output.is_empty() {
            self.status = "Choose a Nesstar file, its DDI XML, and an output CSV path.".into();
            return;
        }
        match env::current_exe().and_then(|executable| {
            Command::new(executable)
                .args(["--worker", &self.input, &self.ddi, &self.output])
                .spawn()
        }) {
            Ok(worker) => {
                self.worker = Some(worker);
                self.status = "Converting…".into();
            }
            Err(error) => self.status = format!("Could not start conversion worker: {error}"),
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
                    "Conversion complete. Your CSV is ready.".into()
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
            ui.label("Convert a Nesstar survey and its DDI metadata to CSV.");
            ui.add_space(12.0);
            file_row(
                ui,
                "Nesstar file",
                &mut self.input,
                "Choose Nesstar file",
                false,
            );
            file_row(ui, "DDI XML", &mut self.ddi, "Choose DDI XML", false);
            file_row(
                ui,
                "Output CSV",
                &mut self.output,
                "Choose output CSV",
                true,
            );
            ui.add_space(12.0);
            let busy = self.worker.is_some();
            if ui
                .add_enabled(!busy, egui::Button::new("Convert to CSV"))
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
