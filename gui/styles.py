STYLE_SHEET = """
/* Global Font & Window Background */
QWidget {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    font-size: 13px;
    color: #1F2937; /* Gray 800 */
}

QMainWindow {
    background-color: #F9FAFB; /* Gray 50 */
}

/* Header Section */
#HeaderTitle {
    font-size: 18px;
    font-weight: bold;
    color: #111827; /* Gray 900 */
}

#HeaderSubtitle {
    font-size: 13px;
    color: #6B7280; /* Gray 500 */
}

/* Drag & Drop Zone */
#DropZone {
    background-color: #FFFFFF;
    border: 2px dashed #D1D5DB; /* Gray 300 */
    border-radius: 8px;
}

#DropZone[dragActive="true"] {
    background-color: #EFF6FF; /* Blue 50 */
    border: 2px dashed #3B82F6; /* Blue 500 */
}

#DropZoneLabel {
    font-size: 14px;
    color: #4B5563; /* Gray 600 */
    font-weight: 500;
}

#DropZoneSublabel {
    font-size: 11px;
    color: #9CA3AF; /* Gray 400 */
}

/* List Widgets & Panels */
QListWidget {
    background-color: #FFFFFF;
    border: 1px solid #E5E7EB; /* Gray 200 */
    border-radius: 6px;
    padding: 4px;
}

QListWidget::item {
    background-color: #F3F4F6; /* Gray 100 */
    border-radius: 4px;
    padding: 8px;
    margin-bottom: 4px;
}

QListWidget::item:hover {
    background-color: #E5E7EB; /* Gray 200 */
}

/* Buttons */
QPushButton {
    background-color: #FFFFFF;
    border: 1px solid #D1D5DB;
    border-radius: 6px;
    padding: 6px 12px;
    font-weight: 500;
    outline: none;
}

QPushButton:hover {
    background-color: #F9FAFB;
    border-color: #9CA3AF;
}

QPushButton:pressed {
    background-color: #F3F4F6;
}

QPushButton:disabled {
    background-color: #F3F4F6;
    color: #9CA3AF;
    border-color: #E5E7EB;
}

/* Primary Convert Button */
#ConvertButton {
    background-color: #16A34A; /* Green 600 */
    color: #FFFFFF;
    border: none;
    font-size: 14px;
    font-weight: 600;
    padding: 10px 20px;
    border-radius: 6px;
}

#ConvertButton:hover {
    background-color: #15803D; /* Green 700 */
}

#ConvertButton:pressed {
    background-color: #166534; /* Green 800 */
}

#ConvertButton:disabled {
    background-color: #E5E7EB; /* Gray 200 */
    color: #9CA3AF;
}

/* Preview / Folder secondary buttons */
.SecondaryButton {
    background-color: #EFF6FF; /* Blue 50 */
    border: 1px solid #BFDBFE; /* Blue 200 */
    color: #2563EB; /* Blue 600 */
}

.SecondaryButton:hover {
    background-color: #DBEAFE; /* Blue 100 */
    border-color: #93C5FD;
}

.SecondaryButton:pressed {
    background-color: #BFDBFE;
}

/* Format Card Checkboxes */
QGroupBox {
    border: 1px solid #E5E7EB;
    border-radius: 6px;
    margin-top: 12px;
    padding-top: 12px;
    font-weight: 600;
    background-color: #FFFFFF;
}

QGroupBox::title {
    subcontrol-origin: margin;
    subcontrol-position: top left;
    left: 12px;
    padding: 0 4px;
    background-color: #FFFFFF;
    color: #374151;
}

QCheckBox {
    spacing: 8px;
    outline: none;
}

QCheckBox::indicator {
    width: 18px;
    height: 18px;
    border: 1px solid #D1D5DB;
    border-radius: 4px;
    background-color: #FFFFFF;
}

QCheckBox::indicator:hover {
    border-color: #9CA3AF;
    background-color: #F9FAFB;
}

QCheckBox::indicator:checked {
    border-color: #2563EB;
    background-color: #2563EB;
}

/* Progress bar styling */
QProgressBar {
    border: 1px solid #E5E7EB;
    border-radius: 6px;
    text-align: center;
    background-color: #F3F4F6;
    height: 16px;
    font-size: 10px;
    font-weight: bold;
    color: #1F2937;
}

QProgressBar::chunk {
    background-color: #3B82F6; /* Blue 500 */
    border-radius: 5px;
}

/* ScrollBars */
QScrollBar:vertical {
    border: none;
    background: #F3F4F6;
    width: 8px;
    margin: 0px 0px 0px 0px;
}

QScrollBar::handle:vertical {
    background: #C5C7CC;
    border-radius: 4px;
    min-height: 20px;
}

QScrollBar::handle:vertical:hover {
    background: #A0A2A6;
}

QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    border: none;
    background: none;
    height: 0px;
}

QScrollBar:horizontal {
    border: none;
    background: #F3F4F6;
    height: 8px;
    margin: 0px 0px 0px 0px;
}

QScrollBar::handle:horizontal {
    background: #C5C7CC;
    border-radius: 4px;
    min-width: 20px;
}

QScrollBar::handle:horizontal:hover {
    background: #A0A2A6;
}

QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {
    border: none;
    background: none;
    width: 0px;
}

/* Table View in Preview Window */
QTableView {
    background-color: #FFFFFF;
    border: 1px solid #E5E7EB;
    gridline-color: #F3F4F6;
    selection-background-color: #DBEAFE;
    selection-color: #1E40AF;
    outline: none;
}

QHeaderView::section {
    background-color: #F9FAFB;
    color: #4B5563;
    padding: 6px 12px;
    border: none;
    border-bottom: 1px solid #E5E7EB;
    border-right: 1px solid #E5E7EB;
    font-weight: 600;
    font-size: 11px;
}

QTableView QTableCornerButton::section {
    background-color: #F9FAFB;
    border: none;
    border-bottom: 1px solid #E5E7EB;
    border-right: 1px solid #E5E7EB;
}

/* Status Badges */
#SuccessLabel {
    color: #16A34A;
    font-weight: 600;
}

#ErrorLabel {
    color: #DC2626;
    font-weight: 600;
}

#MissingLabel {
    color: #D97706;
    font-weight: 600;
}
"""
