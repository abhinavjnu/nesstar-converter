use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::Duration,
};

use eframe::egui;
use nesstar_core::pipeline::OutputFormat;

const ALL_FORMATS: &[OutputFormat] = &[
    OutputFormat::Csv,
    OutputFormat::Parquet,
    OutputFormat::Dta,
    OutputFormat::Spss,
    OutputFormat::Jsonl,
    OutputFormat::Json,
    OutputFormat::Tsv,
    OutputFormat::Fwf,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuiTab {
    Single,
    Batch,
    Preview,
}

fn main() -> eframe::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("--worker") {
        return worker(&args[1..]);
    }
    eframe::run_native(
        "Nesstar Converter",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([880.0, 620.0])
                .with_min_inner_size([650.0, 450.0]),
            ..Default::default()
        },
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
    active_tab: GuiTab,
    // Single mode
    input: String,
    ddi: String,
    output: String,
    format: OutputFormat,
    worker: Option<Child>,
    status: String,

    // Batch mode
    batch_in_dir: String,
    batch_out_dir: String,
    batch_files: Vec<PathBuf>,
    batch_progress: (usize, usize),
    batch_status: String,

    // Preview mode
    preview_headers: Vec<String>,
    preview_rows: Vec<Vec<String>>,
    preview_loading: bool,
    preview_error: String,
}

impl Default for ConverterApp {
    fn default() -> Self {
        Self {
            active_tab: GuiTab::Single,
            input: String::new(),
            ddi: String::new(),
            output: String::new(),
            format: OutputFormat::Parquet,
            worker: None,
            status: String::new(),

            batch_in_dir: String::new(),
            batch_out_dir: String::new(),
            batch_files: Vec::new(),
            batch_progress: (0, 0),
            batch_status: String::new(),

            preview_headers: Vec::new(),
            preview_rows: Vec::new(),
            preview_loading: false,
            preview_error: String::new(),
        }
    }
}

impl ConverterApp {
    /// Ensure the output path extension matches the selected format.
    fn sync_output_extension(&mut self) {
        if self.output.is_empty() {
            return;
        }
        let path = Path::new(&self.output);
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

    /// Auto-detect DDI XML if not explicitly set
    fn auto_detect_ddi(&mut self) {
        if self.input.is_empty() {
            return;
        }
        let input_path = Path::new(&self.input);
        if let Some(parent) = input_path.parent() {
            let stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let candidates = [
                parent.join(format!("{stem}.ddi.xml")),
                parent.join(format!("{stem}.xml")),
                parent.join("ddi.xml"),
            ];
            for cand in candidates {
                if cand.exists() {
                    self.ddi = cand.display().to_string();
                    break;
                }
            }
        }
    }

    fn start_single(&mut self) {
        if self.input.is_empty() || self.output.is_empty() {
            self.status = "Please choose a Nesstar file and an output destination.".into();
            return;
        }
        if self.ddi.is_empty() {
            self.auto_detect_ddi();
        }
        let ddi_to_pass = if self.ddi.is_empty() {
            "auto"
        } else {
            &self.ddi
        };

        match env::current_exe().and_then(|exe| {
            Command::new(exe)
                .args(["--worker", &self.input, ddi_to_pass, &self.output])
                .spawn()
        }) {
            Ok(child) => {
                self.worker = Some(child);
                self.status = format!("Converting to {} …", self.format.label());
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
                    format!("✓ Conversion complete! {} is ready.", self.format.label())
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

    fn scan_batch_dir(&mut self) {
        if self.batch_in_dir.is_empty() {
            return;
        }
        let p = Path::new(&self.batch_in_dir);
        let mut found = Vec::new();
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("nesstar"))
                        .unwrap_or(false)
                {
                    found.push(path);
                }
            }
        }
        self.batch_files = found;
        self.batch_status = format!("Found {} .Nesstar file(s).", self.batch_files.len());
    }

    fn run_batch(&mut self) {
        if self.batch_files.is_empty() || self.batch_out_dir.is_empty() {
            self.batch_status =
                "Select input directory with .Nesstar files and an output folder.".into();
            return;
        }
        let out_dir = PathBuf::from(&self.batch_out_dir);
        let _ = fs::create_dir_all(&out_dir);
        let total = self.batch_files.len();
        let mut success = 0;

        for (idx, nesstar) in self.batch_files.iter().enumerate() {
            self.batch_progress = (idx + 1, total);
            let stem = nesstar
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("survey");
            let out_file = out_dir.join(format!("{stem}.{}", self.format.extension()));

            if let Ok(()) =
                nesstar_core::pipeline::convert(nesstar, "auto", &out_file, 10_000, || true)
            {
                success += 1;
            }
        }
        self.batch_status = format!(
            "✓ Batch complete! Successfully converted {success}/{total} files to {}.",
            self.format.label()
        );
    }

    fn load_preview(&mut self) {
        if self.input.is_empty() {
            self.preview_error = "Select a .Nesstar file first.".into();
            return;
        }
        self.preview_loading = true;
        self.preview_error.clear();
        self.preview_headers.clear();
        self.preview_rows.clear();

        if self.ddi.is_empty() {
            self.auto_detect_ddi();
        }
        let ddi_arg = if self.ddi.is_empty() {
            "auto"
        } else {
            &self.ddi
        };

        // Quick memory conversion of top 50 records to temp CSV
        let tmp_csv = std::env::temp_dir().join(format!("nesstar_prev_{}.csv", std::process::id()));
        let _ = fs::remove_file(&tmp_csv);

        match nesstar_core::pipeline::convert(&self.input, ddi_arg, &tmp_csv, 50, || true) {
            Ok(()) => {
                if let Ok(content) = fs::read_to_string(&tmp_csv) {
                    let mut lines = content.lines();
                    if let Some(header_line) = lines.next() {
                        self.preview_headers =
                            header_line.split(',').map(|s| s.to_string()).collect();
                    }
                    for line in lines.take(50) {
                        let row: Vec<String> = line.split(',').map(|s| s.to_string()).collect();
                        self.preview_rows.push(row);
                    }
                }
                let _ = fs::remove_file(&tmp_csv);
            }
            Err(e) => {
                self.preview_error = format!("Preview failed: {e}");
            }
        }
        self.preview_loading = false;
    }
}

impl eframe::App for ConverterApp {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        self.poll_worker();
        if self.worker.is_some() {
            context.request_repaint_after(Duration::from_millis(200));
        }

        // ── Brand & Support Side Panel ───────────────────────────────────
        egui::SidePanel::left("brand_panel")
            .resizable(false)
            .default_width(210.0)
            .show(context, |ui| {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.heading("Nesstar Converter");
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(14.0);

                    ui.small("NAVIGATION");
                    ui.add_space(6.0);

                    ui.selectable_value(&mut self.active_tab, GuiTab::Single, "Single File");
                    ui.selectable_value(&mut self.active_tab, GuiTab::Batch, "Batch Directory");
                    ui.selectable_value(&mut self.active_tab, GuiTab::Preview, "Data Preview");

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Builder Credits
                    ui.small("DEVELOPED BY");
                    ui.add_space(2.0);
                    ui.strong("Abhinav Kumar");
                    ui.hyperlink_to("@abhinavjnu", "https://github.com/abhinavjnu");

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(16.0);

                    ui.small("SUPPORT THE PROJECT");
                    ui.add_space(8.0);

                    if ui
                        .add_sized([150.0, 28.0], egui::Button::new("Sponsor on GitHub"))
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                            "https://github.com/sponsors/abhinavjnu",
                        ));
                    }
                    ui.add_space(6.0);
                    if ui
                        .add_sized([150.0, 28.0], egui::Button::new("Buy Me a Coffee"))
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                            "https://buymeacoffee.com/abhinavjnu",
                        ));
                    }
                });
            });

        // ── Main Content Workspace ────────────────────────────────────────
        egui::CentralPanel::default().show(context, |ui| match self.active_tab {
            GuiTab::Single => self.render_single_tab(ui),
            GuiTab::Batch => self.render_batch_tab(ui),
            GuiTab::Preview => self.render_preview_tab(ui),
        });
    }
}

impl ConverterApp {
    fn render_single_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Single File Conversion");
        ui.label("Convert a proprietary .Nesstar binary dataset to open data formats.");
        ui.add_space(14.0);

        // Input row
        ui.horizontal(|ui| {
            ui.label("Nesstar file:");
            ui.text_edit_singleline(&mut self.input);
            if ui.button("Browse…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Nesstar Survey", &["Nesstar", "nesstar"])
                    .pick_file()
            {
                self.input = path.display().to_string();
                self.auto_detect_ddi();
                if self.output.is_empty() {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output");
                    if let Some(parent) = path.parent() {
                        self.output = parent
                            .join(format!("{stem}.{}", self.format.extension()))
                            .display()
                            .to_string();
                    }
                }
            }
        });

        // DDI row
        ui.horizontal(|ui| {
            ui.label("DDI XML (auto):");
            ui.text_edit_singleline(&mut self.ddi);
            if ui.button("Browse…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("DDI Metadata", &["xml", "ddi"])
                    .pick_file()
            {
                self.ddi = path.display().to_string();
            }
        });
        ui.add_space(10.0);

        // Format selector
        ui.horizontal(|ui| {
            ui.label("Output Format:");
            let prev = self.format;
            egui::ComboBox::from_id_salt("format_picker_single")
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
        ui.add_space(8.0);

        // Output destination
        ui.horizontal(|ui| {
            ui.label("Destination: ");
            ui.text_edit_singleline(&mut self.output);
            if ui.button("Save As…").clicked() {
                let ext = self.format.extension();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(self.format.label(), &[ext])
                    .save_file()
                {
                    self.output = path.display().to_string();
                    if !self.output.ends_with(&format!(".{ext}")) {
                        self.output.push('.');
                        self.output.push_str(ext);
                    }
                }
            }
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        // Convert Button
        let busy = self.worker.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(format!("Convert to {}", self.format.label())),
                )
                .clicked()
            {
                self.start_single();
            }
            if busy {
                ui.spinner();
            }
        });

        if !self.status.is_empty() {
            ui.add_space(10.0);
            ui.label(&self.status);
        }
    }

    fn render_batch_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Batch Directory Conversion");
        ui.label("Convert multiple survey rounds in a single operation.");
        ui.add_space(14.0);

        ui.horizontal(|ui| {
            ui.label("Input Directory: ");
            ui.text_edit_singleline(&mut self.batch_in_dir);
            if ui.button("Select Folder…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                self.batch_in_dir = path.display().to_string();
                self.scan_batch_dir();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Output Directory:");
            ui.text_edit_singleline(&mut self.batch_out_dir);
            if ui.button("Select Folder…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                self.batch_out_dir = path.display().to_string();
            }
        });
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Batch Target Format:");
            egui::ComboBox::from_id_salt("format_picker_batch")
                .selected_text(self.format.label())
                .show_ui(ui, |ui| {
                    for &fmt in ALL_FORMATS {
                        ui.selectable_value(&mut self.format, fmt, fmt.label());
                    }
                });
        });

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(12.0);

        if ui
            .button(format!("Convert All ({}) Files", self.batch_files.len()))
            .clicked()
        {
            self.run_batch();
        }

        if !self.batch_status.is_empty() {
            ui.add_space(10.0);
            ui.label(&self.batch_status);
        }
    }

    fn render_preview_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Data Preview");
        ui.label("In-memory preview of the first 50 survey records.");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.button("Load Preview").clicked() {
                self.load_preview();
            }
            if self.preview_loading {
                ui.spinner();
            }
        });

        if !self.preview_error.is_empty() {
            ui.colored_label(egui::Color32::RED, &self.preview_error);
        }

        if !self.preview_headers.is_empty() {
            ui.add_space(8.0);
            ui.label(format!(
                "Columns: {} | Previewing first {} rows",
                self.preview_headers.len(),
                self.preview_rows.len()
            ));
            ui.add_space(6.0);

            egui::ScrollArea::both().max_height(400.0).show(ui, |ui| {
                egui::Grid::new("preview_table_grid")
                    .striped(true)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        // Headers
                        for h in &self.preview_headers {
                            ui.strong(h);
                        }
                        ui.end_row();

                        // Rows
                        for row in &self.preview_rows {
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                    });
            });
        }
    }
}
