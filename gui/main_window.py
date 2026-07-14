import os
import platform
import subprocess
from pathlib import Path
from PySide6.QtWidgets import (QMainWindow, QWidget, QVBoxLayout, QHBoxLayout, 
                             QLabel, QPushButton, QListWidget, QListWidgetItem, 
                             QGroupBox, QCheckBox, QProgressBar, QTextEdit, 
                             QFileDialog, QMessageBox, QFrame, QGridLayout)
from PySide6.QtCore import Slot, Signal, QSize

from gui.drop_zone import DropZone
from gui.resources import (get_icon_from_svg, DATABASE_REFRESH_SVG,
                           FILE_ICON_SVG, SUCCESS_ICON_SVG, WARNING_ICON_SVG,
                           CLOSE_ICON_SVG)
from gui.styles import STYLE_SHEET
from gui.converter_thread import ConverterThread


def find_ddi_for_nesstar(nesstar_path: str) -> str | None:
    """Detect companion DDI XML for a .Nesstar file.
    
    Search strategy (in order):
    1. Same directory: exact 'ddi.xml' (case-insensitive)
    2. Same directory: any .xml file whose stem matches the .Nesstar stem
    3. Same directory: any .xml file at all (if there's exactly one)
    4. Parent directory: repeat steps 1-3
    """
    nes_path = Path(nesstar_path)
    nes_stem = nes_path.stem.lower()  # e.g. 'DDI-IND-CSO-PLFS-2017-18'
    
    def _search_dir(search_dir: Path) -> str | None:
        if not search_dir.is_dir():
            return None
        try:
            xml_files = [f for f in search_dir.iterdir() if f.suffix.lower() == '.xml']
        except OSError:
            return None
        
        # 1. Exact 'ddi.xml'
        for f in xml_files:
            if f.name.lower() == 'ddi.xml':
                return str(f)
        
        # 2. Matching stem (e.g. DDI-IND-CSO-PLFS-2017-18.xml for DDI-IND-CSO-PLFS-2017-18.Nesstar)
        for f in xml_files:
            if f.stem.lower() == nes_stem:
                return str(f)
        
        # 3. Single XML file in the directory
        if len(xml_files) == 1:
            return str(xml_files[0])
        
        return None
    
    # Search same directory first
    result = _search_dir(nes_path.parent)
    if result:
        return result
    
    # Search parent directory (MoSPI packages often have DDI XML one level up)
    result = _search_dir(nes_path.parent.parent)
    if result:
        return result
    
    return None


class FileListItemWidget(QWidget):
    """Custom widget for items in the file list."""
    removeRequested = Signal(str) # Emits filepath to be removed
    ddiUpdated = Signal(str, str)  # Emits (nesstar_path, new_ddi_path)
    
    def __init__(self, nesstar_path: str, parent=None):
        super().__init__(parent)
        self.nesstar_path = nesstar_path
        self.ddi_path = find_ddi_for_nesstar(nesstar_path)
        
        layout = QHBoxLayout(self)
        layout.setContentsMargins(6, 4, 6, 4)
        layout.setSpacing(10)
        
        # File Icon
        file_icon = QLabel(self)
        file_icon.setPixmap(get_icon_from_svg(FILE_ICON_SVG, 18).pixmap(18, 18))
        layout.addWidget(file_icon)
        
        # Nesstar Name
        self.name_label = QLabel(os.path.basename(nesstar_path), self)
        self.name_label.setStyleSheet("font-weight: 500;")
        layout.addWidget(self.name_label)
        
        layout.addSpacing(10)
        
        # DDI Info
        self.ddi_label = QLabel(self)
        self.locate_btn = QPushButton("Locate DDI XML", self)
        self.locate_btn.clicked.connect(self.browse_ddi)
        self.locate_btn.setStyleSheet("font-size: 11px; padding: 2px 6px;")
        
        layout.addWidget(self.ddi_label)
        layout.addWidget(self.locate_btn)
        
        layout.addStretch()
        
        # Remove button
        remove_btn = QPushButton(self)
        remove_btn.setIcon(get_icon_from_svg(CLOSE_ICON_SVG, 16))
        remove_btn.setFlat(True)
        remove_btn.setFixedSize(24, 24)
        remove_btn.setStyleSheet("border: none; padding: 0;")
        remove_btn.clicked.connect(lambda: self.window().remove_file(self.nesstar_path))
        layout.addWidget(remove_btn)
        
        self.update_ddi_ui()
        
    def update_ddi_ui(self):
        if self.ddi_path:
            self.ddi_label.setText(f"XML: {os.path.basename(self.ddi_path)}")
            self.ddi_label.setStyleSheet("color: #16A34A; font-size: 11px;")
            self.locate_btn.hide()
        else:
            self.ddi_label.setText("Companion ddi.xml missing!")
            self.ddi_label.setStyleSheet("color: #D97706; font-size: 11px; font-weight: bold;")
            self.locate_btn.show()
            
    def browse_ddi(self):
        file, _ = QFileDialog.getOpenFileName(
            self,
            f"Select companion DDI XML for {os.path.basename(self.nesstar_path)}",
            os.path.dirname(self.nesstar_path),
            "XML Files (*.xml);;All Files (*)"
        )
        if file:
            self.ddi_path = file
            self.update_ddi_ui()
            # Notify main window
            main_win = self.window()
            if hasattr(main_win, 'update_file_ddi'):
                main_win.update_file_ddi(self.nesstar_path, self.ddi_path)


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Nesstar Converter")
        self.resize(950, 750)
        self.setMinimumSize(800, 650)
        
        self.selected_files = {} # path -> ddi_path
        self.conversion_results = {} # path -> report_dict
        
        self.init_ui()
        self.setStyleSheet(STYLE_SHEET)
        
    def init_ui(self):
        central_widget = QWidget(self)
        self.setCentralWidget(central_widget)
        
        main_layout = QVBoxLayout(central_widget)
        main_layout.setContentsMargins(20, 20, 20, 20)
        main_layout.setSpacing(16)
        
        # 1. Header
        header_layout = QHBoxLayout()
        header_layout.setSpacing(12)
        
        logo = QLabel(self)
        logo.setPixmap(get_icon_from_svg(DATABASE_REFRESH_SVG, 36).pixmap(36, 36))
        header_layout.addWidget(logo)
        
        title_layout = QVBoxLayout()
        title_layout.setSpacing(2)
        
        title = QLabel("Nesstar Converter", self)
        title.setObjectName("HeaderTitle")
        title_layout.addWidget(title)
        
        subtitle = QLabel("Convert proprietary Nesstar survey data to open formats", self)
        subtitle.setObjectName("HeaderSubtitle")
        title_layout.addWidget(subtitle)
        
        header_layout.addLayout(title_layout)
        header_layout.addStretch()
        main_layout.addLayout(header_layout)
        
        # Divider line
        divider = QFrame(self)
        divider.setFrameShape(QFrame.HLine)
        divider.setFrameShadow(QFrame.Sunken)
        divider.setStyleSheet("background-color: #E5E7EB;")
        main_layout.addWidget(divider)
        
        # 2. Drag & Drop Zone
        self.drop_zone = DropZone(self)
        self.drop_zone.setFixedHeight(120)
        self.drop_zone.filesDropped.connect(self.add_files)
        main_layout.addWidget(self.drop_zone)
        
        # 3. File List Area
        self.file_list_label = QLabel("Files to convert (0):", self)
        self.file_list_label.setStyleSheet("font-weight: bold;")
        main_layout.addWidget(self.file_list_label)
        
        self.file_list_widget = QListWidget(self)
        self.file_list_widget.setFixedHeight(120)
        main_layout.addWidget(self.file_list_widget)
        
        # 4. Format Picker
        format_group = QGroupBox("Select Output Formats", self)
        format_layout = QGridLayout(format_group)
        format_layout.setContentsMargins(12, 16, 12, 12)
        format_layout.setSpacing(10)
        
        # Formats list
        self.format_checkboxes = {}
        formats = [
            ('csv', 'CSV (.csv)', True),
            ('excel', 'Excel (.xlsx)', True),
            ('stata', 'Stata (.dta)', False),
            ('parquet', 'Parquet (.parquet)', True),
            ('json', 'JSON (.json)', False),
            ('jsonl', 'JSON Lines (.jsonl)', False),
            ('tsv', 'TSV (.tsv)', False),
            ('fwf', 'Fixed-Width (.txt)', False)
        ]
        
        for idx, (fmt_key, fmt_name, default_checked) in enumerate(formats):
            cb = QCheckBox(fmt_name, format_group)
            cb.setChecked(default_checked)
            cb.stateChanged.connect(self.validate_inputs)
            self.format_checkboxes[fmt_key] = cb
            
            row = idx // 4
            col = idx % 4
            format_layout.addWidget(cb, row, col)
            
        main_layout.addWidget(format_group)
        
        # 5. Output Directory Selector
        out_layout = QHBoxLayout()
        out_layout.setSpacing(10)
        
        self.save_source_cb = QCheckBox("Save in the same folder as source files", self)
        self.save_source_cb.setChecked(True)
        self.save_source_cb.stateChanged.connect(self.toggle_output_dir_picker)
        out_layout.addWidget(self.save_source_cb)
        
        self.out_dir_label = QLabel("Output Folder:", self)
        self.out_dir_label.hide()
        out_layout.addWidget(self.out_dir_label)
        
        self.out_dir_text = QLabel(self)
        self.out_dir_text.setStyleSheet("background-color: #F3F4F6; padding: 4px 8px; border-radius: 4px; border: 1px solid #E5E7EB;")
        self.out_dir_text.hide()
        out_layout.addWidget(self.out_dir_text)
        
        self.out_dir_btn = QPushButton("Browse...", self)
        self.out_dir_btn.clicked.connect(self.browse_output_dir)
        self.out_dir_btn.hide()
        out_layout.addWidget(self.out_dir_btn)
        
        out_layout.addStretch()
        main_layout.addLayout(out_layout)
        
        # 6. Action Button
        self.convert_btn = QPushButton("Convert Files", self)
        self.convert_btn.setObjectName("ConvertButton")
        self.convert_btn.clicked.connect(self.start_conversion)
        main_layout.addWidget(self.convert_btn)
        
        # 7. Progress & Log Area (Hidden by default)
        self.progress_container = QWidget(self)
        progress_layout = QVBoxLayout(self.progress_container)
        progress_layout.setContentsMargins(0, 0, 0, 0)
        progress_layout.setSpacing(8)
        
        self.progress_label = QLabel("Ready", self.progress_container)
        progress_layout.addWidget(self.progress_label)
        
        bar_layout = QHBoxLayout()
        self.progress_bar = QProgressBar(self.progress_container)
        self.progress_bar.setValue(0)
        bar_layout.addWidget(self.progress_bar)
        
        self.abort_btn = QPushButton("Abort", self.progress_container)
        self.abort_btn.clicked.connect(self.abort_conversion)
        self.abort_btn.setStyleSheet("background-color: #DC2626; color: white; border: none; font-weight: bold; padding: 4px 12px; border-radius: 4px;")
        bar_layout.addWidget(self.abort_btn)
        progress_layout.addLayout(bar_layout)
        
        self.log_widget = QTextEdit(self.progress_container)
        self.log_widget.setReadOnly(True)
        self.log_widget.setFixedHeight(100)
        self.log_widget.setStyleSheet("font-family: monospace; font-size: 11px; background-color: #1F2937; color: #F9FAFB;")
        progress_layout.addWidget(self.log_widget)
        
        self.progress_container.hide()
        main_layout.addWidget(self.progress_container)
        
        # 8. Results View (Hidden by default)
        self.results_container = QWidget(self)
        results_layout = QVBoxLayout(self.results_container)
        results_layout.setContentsMargins(0, 0, 0, 0)
        results_layout.setSpacing(8)
        
        self.results_label = QLabel("Conversion Results:", self.results_container)
        self.results_label.setStyleSheet("font-weight: bold;")
        results_layout.addWidget(self.results_label)
        
        self.results_list = QListWidget(self.results_container)
        self.results_list.setFixedHeight(120)
        results_layout.addWidget(self.results_list)
        
        self.results_container.hide()
        main_layout.addWidget(self.results_container)
        
        self.validate_inputs()
        
    def toggle_output_dir_picker(self, state):
        is_checked = self.save_source_cb.isChecked()
        self.out_dir_label.setVisible(not is_checked)
        self.out_dir_text.setVisible(not is_checked)
        self.out_dir_btn.setVisible(not is_checked)
        self.validate_inputs()
        
    def browse_output_dir(self):
        dir_path = QFileDialog.getExistingDirectory(self, "Select Output Directory", "")
        if dir_path:
            self.out_dir_text.setText(dir_path)
            self.validate_inputs()
            
    def add_files(self, filepaths):
        for path in filepaths:
            if path not in self.selected_files:
                ddi = find_ddi_for_nesstar(path)
                self.selected_files[path] = ddi
                
                # Add to QListWidget
                item = QListWidgetItem(self.file_list_widget)
                widget = FileListItemWidget(path, self.file_list_widget)
                item.setSizeHint(QSize(widget.sizeHint().width(), 44))
                self.file_list_widget.addItem(item)
                self.file_list_widget.setItemWidget(item, widget)
                
        self.update_file_count_label()
        self.validate_inputs()
        
    def remove_file(self, filepath):
        if filepath in self.selected_files:
            del self.selected_files[filepath]
            
            # Find item in list widget and remove
            for r in range(self.file_list_widget.count()):
                item = self.file_list_widget.item(r)
                widget = self.file_list_widget.itemWidget(item)
                if widget and widget.nesstar_path == filepath:
                    self.file_list_widget.takeItem(r)
                    break
                    
        self.update_file_count_label()
        self.validate_inputs()
        
    def update_file_ddi(self, nesstar_path, ddi_path):
        if nesstar_path in self.selected_files:
            self.selected_files[nesstar_path] = ddi_path
            self.validate_inputs()
            
    def update_file_count_label(self):
        self.file_list_label.setText(f"Files to convert ({len(self.selected_files)}):")
        
    def get_selected_formats(self):
        return [fmt for fmt, cb in self.format_checkboxes.items() if cb.isChecked()]
        
    def validate_inputs(self):
        # Must have at least 1 file
        has_files = len(self.selected_files) > 0
        
        # Must have at least 1 format checked
        has_formats = len(self.get_selected_formats()) > 0
        
        # If save_source is unchecked, must have selected an output directory
        has_valid_out = True
        if not self.save_source_cb.isChecked():
            has_valid_out = bool(self.out_dir_text.text())
            
        # Check if all files have their companion DDI XML loaded
        all_have_ddi = all(self.selected_files.values())
        
        # Enable/Disable convert button
        self._can_convert = has_files and has_formats and has_valid_out and all_have_ddi
        self.convert_btn.setEnabled(self._can_convert)
        
        # Set explanatory tooltips/status message if disabled
        if not has_files:
            self.convert_btn.setToolTip("Please drag or select at least one .Nesstar file to convert.")
        elif not all_have_ddi:
            self.convert_btn.setToolTip("All files must have an associated DDI XML file. Please locate the missing DDI files.")
        elif not has_formats:
            self.convert_btn.setToolTip("Please check at least one output format.")
        elif not has_valid_out:
            self.convert_btn.setToolTip("Please browse and select an output folder.")
        else:
            self.convert_btn.setToolTip("Start the conversion process.")
            
    def start_conversion(self):
        if not self._can_convert:
            return
            
        # Formulate tasks list
        tasks = []
        formats = self.get_selected_formats()
        
        for nesstar_path, ddi_path in self.selected_files.items():
            if self.save_source_cb.isChecked():
                # Save in same folder
                out_dir = str(Path(nesstar_path).parent)
            else:
                out_dir = self.out_dir_text.text()
                
            tasks.append({
                'nesstar': nesstar_path,
                'ddi': ddi_path,
                'output_dir': out_dir,
                'formats': formats
            })
            
        # Reset UI for conversion progress
        self.progress_container.show()
        self.results_container.hide()
        self.log_widget.clear()
        self.progress_bar.setValue(0)
        self.progress_label.setText("Preparing conversion...")
        self.convert_btn.setEnabled(False)
        self.drop_zone.setEnabled(False)
        self.file_list_widget.setEnabled(False)
        self.abort_btn.setEnabled(True)
        
        # Spawn thread
        self.thread = ConverterThread(tasks, self)
        self.thread.progress_message.connect(self.append_log)
        self.thread.file_started.connect(self.on_file_started)
        self.thread.file_completed.connect(self.on_file_completed)
        self.thread.file_failed.connect(self.on_file_failed)
        self.thread.all_finished.connect(self.on_all_finished)
        self.thread.start()
        
    @Slot(str)
    def append_log(self, msg):
        self.log_widget.append(msg)
        
    @Slot(str, int, int)
    def on_file_started(self, filename, current, total):
        self.progress_label.setText(f"Converting file {current} of {total}: {filename}...")
        pct = int(((current - 1) / total) * 100)
        self.progress_bar.setValue(pct)
        
    @Slot(str, dict)
    def on_file_completed(self, filename, report):
        self.log_widget.append(f"SUCCESS: {filename} converted successfully.")
        
    @Slot(str, str)
    def on_file_failed(self, filename, err):
        self.log_widget.append(f"FAILED: {filename} failed: {err}")
        
    @Slot(list)
    def on_all_finished(self, results):
        self.progress_bar.setValue(100)
        self.progress_label.setText("Conversion process finished.")
        
        # Enable UI
        self.convert_btn.setEnabled(True)
        self.drop_zone.setEnabled(True)
        self.file_list_widget.setEnabled(True)
        self.abort_btn.setEnabled(True)
        
        if hasattr(self, 'thread') and self.thread._is_cancelled:
            self.progress_bar.setValue(0)
            self.progress_label.setText("Conversion aborted.")
            QMessageBox.warning(self, "Conversion Aborted", "The conversion process was aborted by the user.")
            return
        
        # Populate results
        self.results_list.clear()
        self.conversion_results = {}
        
        success_count = 0
        for res in results:
            nesstar_path = res['nesstar']
            filename = os.path.basename(nesstar_path)
            
            item = QListWidgetItem(self.results_list)
            widget = QWidget(self.results_list)
            w_layout = QHBoxLayout(widget)
            w_layout.setContentsMargins(6, 4, 6, 4)
            w_layout.setSpacing(10)
            
            status_icon = QLabel(widget)
            name_lbl = QLabel(filename, widget)
            name_lbl.setStyleSheet("font-weight: 500;")
            
            w_layout.addWidget(status_icon)
            w_layout.addWidget(name_lbl)
            
            if res['status'] == 'success':
                success_count += 1
                report = res['report']
                # Store the report using the nesstar file path
                self.conversion_results[nesstar_path] = report
                
                status_icon.setPixmap(get_icon_from_svg(SUCCESS_ICON_SVG, 16).pixmap(16, 16))
                
                # Fetch output directory to let user open it
                out_dir = report.get('conversion_report.json', {}).get('output_dir', '')
                if not out_dir and 'blocks' in report and report['blocks']:
                    # Fallback to output folder of first block
                    first_block = list(report['blocks'].values())[0]
                    if 'files' in first_block and first_block['files']:
                        first_file = list(first_block['files'].values())[0]
                        out_dir = os.path.dirname(first_file)
                        
                if not out_dir:
                    out_dir = os.path.dirname(nesstar_path)
                    
                w_layout.addStretch()
                
                # Preview button
                # The report structure has 'blocks' -> block_name -> 'files' -> format -> filepath
                # Format the block files dictionary for preview dialog
                block_files = {}
                for b_id, b_info in report.get('blocks', {}).items():
                    b_name = b_info.get('name', f"block_{b_id}")
                    block_files[b_name] = b_info.get('files', {})
                    
                if block_files:
                    prev_btn = QPushButton("Preview Data", widget)
                    prev_btn.setProperty("class", "SecondaryButton")
                    prev_btn.clicked.connect(lambda checked=False, bf=block_files: self.open_preview(bf))
                    prev_btn.setStyleSheet("font-size: 11px; padding: 2px 8px;")
                    w_layout.addWidget(prev_btn)
                
                # Open Folder button
                folder_btn = QPushButton("Open Folder", widget)
                folder_btn.clicked.connect(lambda checked=False, path=out_dir: self.open_output_folder(path))
                folder_btn.setStyleSheet("font-size: 11px; padding: 2px 8px;")
                w_layout.addWidget(folder_btn)
                
            else:
                status_icon.setPixmap(get_icon_from_svg(WARNING_ICON_SVG, 16).pixmap(16, 16))
                err_lbl = QLabel(f"Error: {res['message']}", widget)
                err_lbl.setStyleSheet("color: #DC2626; font-size: 11px;")
                w_layout.addWidget(err_lbl)
                w_layout.addStretch()
                
            item.setSizeHint(QSize(w_layout.sizeHint().width(), 40))
            self.results_list.addItem(item)
            self.results_list.setItemWidget(item, widget)
            
        self.results_container.show()
        
        # Show message dialog
        if success_count == len(results):
            QMessageBox.information(self, "Conversion Complete", f"Successfully converted all {len(results)} files!")
        elif success_count > 0:
            QMessageBox.warning(self, "Conversion Partial", f"Successfully converted {success_count} of {len(results)} files. Review error messages.")
        else:
            QMessageBox.critical(self, "Conversion Failed", "Failed to convert any files. Please check the logs.")
            
    def open_preview(self, block_files):
        # pandas and optional file-format engines are relatively heavy. Import
        # the preview only when requested, after the conversion worker exits.
        from gui.preview_dialog import PreviewDialog

        dialog = PreviewDialog(block_files, self)
        dialog.exec()
        
    def open_output_folder(self, folder_path):
        if not os.path.exists(folder_path):
            QMessageBox.warning(self, "Folder Not Found", f"The output folder does not exist: {folder_path}")
            return
            
        try:
            if platform.system() == "Darwin":
                subprocess.Popen(["open", folder_path])
            elif platform.system() == "Windows":
                os.startfile(folder_path)
            else: # Linux and others
                subprocess.Popen(["xdg-open", folder_path])
        except Exception as e:
            QMessageBox.warning(self, "Error Opening Folder", f"Could not open the folder automatically:\n{str(e)}")
            
    def abort_conversion(self):
        if hasattr(self, 'thread') and self.thread.isRunning():
            reply = QMessageBox.question(
                self,
                "Abort Conversion",
                "Are you sure you want to abort the conversion process?",
                QMessageBox.Yes | QMessageBox.No,
                QMessageBox.No
            )
            if reply == QMessageBox.Yes:
                self.thread.cancel()
                self.progress_label.setText("Aborting conversion...")
                self.abort_btn.setEnabled(False)

