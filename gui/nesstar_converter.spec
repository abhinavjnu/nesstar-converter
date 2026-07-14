# -*- mode: python ; coding: utf-8 -*-
import os
import sys

block_cipher = None

# Base directory is the root of the repository
base_dir = os.path.dirname(os.path.dirname(os.path.abspath(SPEC)))

a = Analysis(
    [os.path.join(base_dir, 'gui', 'app.py')],
    pathex=[base_dir],
    binaries=[],
    datas=[],
    # pandas selects these output engines dynamically. All other application
    # imports are discoverable statically and do not need blanket collection.
    hiddenimports=[
        'openpyxl',
        'pyarrow',
        'pyarrow.parquet',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'tkinter',
        'matplotlib',
        'scipy',
        'IPython',
        'notebook',
        'pytest',
        # The app uses PySide6-Essentials only. Guard against accidentally
        # bundling large add-on modules from a developer's global environment.
        'PySide6.Qt3DCore',
        'PySide6.QtCharts',
        'PySide6.QtDataVisualization',
        'PySide6.QtLocation',
        'PySide6.QtMultimedia',
        'PySide6.QtPdf',
        'PySide6.QtPdfWidgets',
        'PySide6.QtPositioning',
        'PySide6.QtQml',
        'PySide6.QtQuick',
        'PySide6.QtQuickWidgets',
        'PySide6.QtSvg',
        'PySide6.QtSvgWidgets',
        'PySide6.QtVirtualKeyboard',
        'PySide6.QtWebChannel',
        'PySide6.QtWebEngineCore',
        'PySide6.QtWebEngineWidgets',
        'PySide6.QtWebSockets',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# ─── macOS: directory bundle → .app ───
if sys.platform == 'darwin':
    exe = EXE(
        pyz,
        a.scripts,
        [],
        exclude_binaries=True,
        name='NesstarConverter',
        debug=False,
        bootloader_ignore_signals=False,
        strip=False,
        upx=False,       # UPX breaks macOS code signing
        console=False,
        disable_windowed_traceback=False,
        argv_emulation=False,
        target_arch=None,
        codesign_identity=None,
        entitlements_file=None,
    )

    coll = COLLECT(
        exe,
        a.binaries,
        a.zipfiles,
        a.datas,
        strip=False,
        upx=False,
        upx_exclude=[],
        name='NesstarConverter',
    )

    app = BUNDLE(
        coll,
        name='Nesstar Converter.app',
        icon=None,
        bundle_identifier='com.abhinavjnu.nesstar-converter',
        info_plist={
            'CFBundleDisplayName': 'Nesstar Converter',
            'CFBundleShortVersionString': '1.0.4',
            'CFBundleVersion': '1.0.4',
            'NSPrincipalClass': 'NSApplication',
            'NSHighResolutionCapable': True,
        },
    )

# ─── Linux: single-file executable ───
else:
    exe = EXE(
        pyz,
        a.scripts,
        a.binaries,
        a.zipfiles,
        a.datas,
        [],
        name='NesstarConverter',
        debug=False,
        bootloader_ignore_signals=False,
        strip=True,
        upx=True,
        console=False,
        disable_windowed_traceback=False,
        argv_emulation=False,
        target_arch=None,
        codesign_identity=None,
        entitlements_file=None,
    )
