import os
import pandas as pd
from PySide6.QtWidgets import (QDialog, QVBoxLayout, QHBoxLayout, QLabel, 
                             QComboBox, QTableView, QPushButton, QMessageBox)
from PySide6.QtCore import QAbstractTableModel, Qt
from gui.resources import get_icon_from_svg, INFO_ICON_SVG

class DataFrameModel(QAbstractTableModel):
    """A model to link a pandas DataFrame with Qt's QTableView."""
    def __init__(self, df: pd.DataFrame):
        super().__init__()
        self._df = df

    def rowCount(self, parent=None):
        return self._df.shape[0]

    def columnCount(self, parent=None):
        return self._df.shape[1]

    def data(self, index, role=Qt.DisplayRole):
        if not index.isValid():
            return None
            
        if role == Qt.DisplayRole:
            val = self._df.iloc[index.row(), index.column()]
            if pd.isna(val):
                return ""
            return str(val)
            
        return None

    def headerData(self, section, orientation, role=Qt.DisplayRole):
        if role == Qt.DisplayRole:
            if orientation == Qt.Horizontal:
                return str(self._df.columns[section])
            else:
                return str(self._df.index[section] + 1)
        return None


class PreviewDialog(QDialog):
    def __init__(self, block_files: dict, parent=None):
        """
        block_files: dict of {block_name: {format: filepath}}
        e.g., {'household': {'csv': 'path/to/house.csv', 'parquet': 'path/to/house.pqt'}}
        """
        super().__init__(parent)
        self.setWindowTitle("Data Preview — Nesstar Converter")
        self.resize(800, 500)
        self.setMinimumSize(600, 400)
        
        self.block_files = block_files
        self.current_df = None
        
        # Layout
        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(12)
        
        # Top controls
        top_layout = QHBoxLayout()
        top_layout.setSpacing(10)
        
        info_label = QLabel(self)
        info_label.setPixmap(get_icon_from_svg(INFO_ICON_SVG, 20).pixmap(20, 20))
        top_layout.addWidget(info_label)
        
        top_layout.addWidget(QLabel("Select block to preview:", self))
        
        self.block_selector = QComboBox(self)
        self.block_selector.setMinimumWidth(200)
        self.block_selector.currentTextChanged.connect(self.load_selected_block)
        top_layout.addWidget(self.block_selector)
        
        top_layout.addStretch()
        
        self.info_text = QLabel("", self)
        self.info_text.setObjectName("HeaderSubtitle")
        top_layout.addWidget(self.info_text)
        
        layout.addLayout(top_layout)
        
        # Table
        self.table_view = QTableView(self)
        self.table_view.setAlternatingRowColors(True)
        layout.addWidget(self.table_view)
        
        # Close Button
        btn_layout = QHBoxLayout()
        btn_layout.addStretch()
        close_btn = QPushButton("Close", self)
        close_btn.clicked.connect(self.accept)
        btn_layout.addWidget(close_btn)
        layout.addLayout(btn_layout)
        
        # Populate blocks
        self.populate_blocks()
        
    def populate_blocks(self):
        # We prefer CSV or Parquet for preview, but can use others if available
        # Find which blocks actually have previewable files
        self.valid_previews = {}
        for block_name, formats in self.block_files.items():
            # Prefer formats that can be sampled without loading the Arrow
            # engine or an entire file into the GUI process.
            pref_path = None
            pref_fmt = None
            for fmt in ['csv', 'tsv', 'excel', 'parquet', 'json']:
                if fmt in formats and os.path.exists(formats[fmt]):
                    pref_path = formats[fmt]
                    pref_fmt = fmt
                    break
            
            if pref_path:
                self.valid_previews[block_name] = (pref_path, pref_fmt)
                self.block_selector.addItem(block_name)
                
        if not self.valid_previews:
            QMessageBox.warning(self, "Preview Unavailable", "No previewable files (Parquet, CSV, TSV, JSON, or Excel) were found for this conversion.")
            self.reject()
            
    def load_selected_block(self, block_name):
        if block_name not in self.valid_previews:
            return
            
        filepath, fmt = self.valid_previews[block_name]
        
        try:
            # Read first 20 rows of the file
            if fmt == 'parquet':
                # read_parquet does not have simple 'nrows', but we can read with PyArrow or slice
                # Reading whole file or head is usually fast enough for typical survey chunks
                df = pd.read_parquet(filepath).head(20)
            elif fmt == 'csv':
                df = pd.read_csv(filepath, nrows=20)
            elif fmt == 'tsv':
                df = pd.read_csv(filepath, sep='\t', nrows=20)
            elif fmt == 'json':
                df = pd.read_json(filepath).head(20)
            elif fmt == 'excel':
                # openpyxl sheet
                df = pd.read_excel(filepath, nrows=20)
            else:
                raise ValueError("Unsupported preview format")
                
            self.current_df = df
            model = DataFrameModel(df)
            self.table_view.setModel(model)
            
            # Auto resize columns to content with a maximum width limit to avoid huge columns
            self.table_view.resizeColumnsToContents()
            for col in range(df.shape[1]):
                if self.table_view.columnWidth(col) > 250:
                    self.table_view.setColumnWidth(col, 250)
                    
            self.info_text.setText(f"Showing first 20 rows ({df.shape[1]} columns)")
            
        except Exception as e:
            QMessageBox.critical(self, "Preview Error", f"Failed to load preview for {block_name}:\n{str(e)}")
