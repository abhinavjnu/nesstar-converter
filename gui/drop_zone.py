import os
from PySide6.QtWidgets import QFrame, QVBoxLayout, QLabel, QFileDialog
from PySide6.QtCore import Signal, Qt
from PySide6.QtGui import QDragEnterEvent, QDropEvent
from gui.resources import get_icon_from_svg, FILE_ICON_SVG

class DropZone(QFrame):
    filesDropped = Signal(list) # Emitted with list of file paths (str)
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("DropZone")
        self.setAcceptDrops(True)
        self.setProperty("dragActive", False)
        
        # Layout
        layout = QVBoxLayout(self)
        layout.setAlignment(Qt.AlignCenter)
        layout.setSpacing(8)
        
        # Icon Label
        self.icon_label = QLabel(self)
        self.icon_label.setPixmap(get_icon_from_svg(FILE_ICON_SVG, 48).pixmap(48, 48))
        self.icon_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.icon_label)
        
        # Label
        self.text_label = QLabel("Drag & drop .Nesstar files here\nor click to browse", self)
        self.text_label.setObjectName("DropZoneLabel")
        self.text_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.text_label)
        
        # Subtext Label
        self.sub_label = QLabel("Companion ddi.xml files will be auto-detected", self)
        self.sub_label.setObjectName("DropZoneSublabel")
        self.sub_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.sub_label)
        
    def dragEnterEvent(self, event: QDragEnterEvent):
        # Check if the drag contains URLs and at least one has .Nesstar extension
        if event.mimeData().hasUrls():
            has_nesstar = False
            for url in event.mimeData().urls():
                filepath = url.toLocalFile()
                if filepath.lower().endswith('.nesstar'):
                    has_nesstar = True
                    break
            
            if has_nesstar:
                self.setProperty("dragActive", True)
                self.style().unpolish(self)
                self.style().polish(self)
                event.acceptProposedAction()
                return
                
        event.ignore()
        
    def dragLeaveEvent(self, event):
        self.setProperty("dragActive", False)
        self.style().unpolish(self)
        self.style().polish(self)
        event.accept()
        
    def dropEvent(self, event: QDropEvent):
        self.setProperty("dragActive", False)
        self.style().unpolish(self)
        self.style().polish(self)
        
        files = []
        if event.mimeData().hasUrls():
            for url in event.mimeData().urls():
                filepath = url.toLocalFile()
                if filepath.lower().endswith('.nesstar') and os.path.isfile(filepath):
                    files.append(filepath)
                    
        if files:
            self.filesDropped.emit(files)
            event.acceptProposedAction()
        else:
            event.ignore()
            
    def mousePressEvent(self, event):
        # Open file dialog on click
        if event.button() == Qt.LeftButton:
            files, _ = QFileDialog.getOpenFileNames(
                self,
                "Select Nesstar files",
                "",
                "Nesstar Files (*.Nesstar *.nesstar);;All Files (*)"
            )
            if files:
                self.filesDropped.emit(files)
            event.accept()
        else:
            super().mousePressEvent(event)
