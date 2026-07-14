import sys
import os
import subprocess
import json
import traceback
from PySide6.QtCore import QThread, Signal

class ConverterThread(QThread):
    progress_message = Signal(str)            # Current verbose conversion log line
    file_started = Signal(str, int, int)       # filename, current_idx, total_count
    file_completed = Signal(str, dict)         # filename, report_dict
    file_failed = Signal(str, str)             # filename, error_message
    all_finished = Signal(list)                # list of conversion results

    def __init__(self, tasks: list, parent=None):
        """
        tasks: list of dicts:
            [{
                'nesstar': 'path/to/file.Nesstar',
                'ddi': 'path/to/ddi.xml',
                'output_dir': 'path/to/output',
                'formats': ['parquet', 'csv', ...]
            }]
        """
        super().__init__(parent)
        self.tasks = tasks
        self._is_cancelled = False
        self.process = None

    def cancel(self):
        self._is_cancelled = True
        if self.process:
            try:
                # Force kill the subprocess instantly to abort format conversion
                self.process.kill()
            except Exception:
                pass

    def run(self):
        results = []
        total = len(self.tasks)
        
        # Get repository root
        root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        
        for idx, task in enumerate(self.tasks):
            if self._is_cancelled:
                break
                
            nesstar_path = task['nesstar']
            ddi_path = task['ddi']
            output_dir = task['output_dir']
            formats = task['formats']
            
            filename = os.path.basename(nesstar_path)
            self.file_started.emit(filename, idx + 1, total)
            
            # A frozen PyInstaller app reuses its own executable and app.py
            # dispatches the CLI command before importing Qt. In development,
            # invoke the converter module with the current Python interpreter.
            if getattr(sys, "frozen", False):
                cmd = [sys.executable, "convert"]
            else:
                cmd = [sys.executable, "-m", "nesstar_converter", "convert"]
            cmd.extend([
                nesstar_path,
                ddi_path,
                output_dir,
                "--formats",
                ",".join(formats),
            ])
            
            try:
                # Set environment variables for unbuffered I/O. PYTHONPATH is
                # only relevant when running from source; frozen modules are
                # loaded from the PyInstaller bundle.
                env = os.environ.copy()
                env["PYTHONUNBUFFERED"] = "1"
                if not getattr(sys, "frozen", False):
                    if "PYTHONPATH" in env:
                        env["PYTHONPATH"] = root_dir + os.pathsep + env["PYTHONPATH"]
                    else:
                        env["PYTHONPATH"] = root_dir
                
                # Launch the converter in a separate process for minimal GUI overhead
                self.process = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                    env=env,
                    cwd=root_dir
                )
                
                # Stream logs in real-time
                process_stdout = self.process.stdout
                if process_stdout is None:
                    raise RuntimeError("Converter process output was not captured")
                while True:
                    if self._is_cancelled:
                        self.process.kill()
                        break
                    line = process_stdout.readline()
                    if not line:
                        break
                    if line.strip():
                        self.progress_message.emit(line.strip())
                
                self.process.wait()
                
                if self._is_cancelled:
                    results.append({
                        'nesstar': nesstar_path,
                        'status': 'error',
                        'message': 'Conversion was aborted by the user.'
                    })
                    self.file_failed.emit(filename, 'Aborted by user.')
                    break
                    
                if self.process.returncode != 0:
                    err_msg = f"Converter process exited with code {self.process.returncode}"
                    results.append({
                        'nesstar': nesstar_path,
                        'status': 'error',
                        'message': err_msg
                    })
                    self.file_failed.emit(filename, err_msg)
                else:
                    # Load the generated report from output folder
                    report_path = os.path.join(output_dir, "conversion_report.json")
                    if os.path.exists(report_path):
                        with open(report_path, "r") as f:
                            report = json.load(f)
                    else:
                        report = {"blocks": {}, "errors": []}
                        
                    errors = report.get('errors', [])
                    if errors:
                        error_msg = "; ".join(errors)
                        results.append({
                            'nesstar': nesstar_path,
                            'status': 'error',
                            'message': f"Conversion completed with errors: {error_msg}",
                            'report': report
                        })
                        self.file_failed.emit(filename, error_msg)
                    else:
                        results.append({
                            'nesstar': nesstar_path,
                            'status': 'success',
                            'report': report
                        })
                        self.file_completed.emit(filename, report)
                        
            except Exception as e:
                tb = traceback.format_exc()
                results.append({
                    'nesstar': nesstar_path,
                    'status': 'error',
                    'message': str(e),
                    'traceback': tb
                })
                self.file_failed.emit(filename, str(e))
                
        self.all_finished.emit(results)
