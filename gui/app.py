import os
import sys

# Ensure the root of the repository is in sys.path when app.py is run directly.
root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if root_dir not in sys.path:
    sys.path.insert(0, root_dir)

_CONVERTER_COMMANDS = {"info", "convert", "validate", "batch", "formats"}


def _is_converter_invocation() -> bool:
    """Detect CLI arguments passed back to a frozen GUI executable."""
    return len(sys.argv) > 1 and sys.argv[1] in _CONVERTER_COMMANDS


def main():
    # In a PyInstaller build, sys.executable is this application. Conversion
    # workers call it with CLI arguments, so dispatch before importing Qt.
    if _is_converter_invocation():
        from nesstar_converter import main as converter_main

        converter_main()
        return

    # Keep Qt imports out of conversion worker processes.
    from PySide6.QtCore import Qt
    from PySide6.QtWidgets import QApplication

    from gui.main_window import MainWindow
    from gui.resources import DATABASE_REFRESH_SVG, get_icon_from_svg

    QApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )

    app = QApplication(sys.argv)
    app.setApplicationName("Nesstar Converter")
    app.setApplicationDisplayName("Nesstar Converter")
    app.setOrganizationName("abhinavjnu")
    app.setOrganizationDomain("github.com/abhinavjnu")
    
    # Set App Window Icon
    app_icon = get_icon_from_svg(DATABASE_REFRESH_SVG, 64)
    app.setWindowIcon(app_icon)
    
    window = MainWindow()
    window.show()
    
    sys.exit(app.exec())

if __name__ == "__main__":
    main()

