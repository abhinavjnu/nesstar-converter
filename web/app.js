let wasmModule = null;
let nesstarBytes = null;
let ddiXml = null;
let selectedFormat = 'csv';

// Elements
const nesstarInput = document.getElementById('nesstar-input');
const ddiInput = document.getElementById('ddi-input');
const nesstarStatus = document.getElementById('nesstar-status');
const ddiStatus = document.getElementById('ddi-status');
const formatPills = document.getElementById('format-pills');
const previewBtn = document.getElementById('preview-btn');
const convertBtn = document.getElementById('convert-btn');
const statusBox = document.getElementById('status-box');
const previewCard = document.getElementById('preview-card');
const previewMeta = document.getElementById('preview-meta');
const tableContainer = document.getElementById('table-container');

// Format pills handler
formatPills.addEventListener('click', (e) => {
  if (e.target.classList.contains('pill')) {
    document.querySelectorAll('.pill').forEach(p => p.classList.remove('active'));
    e.target.classList.add('active');
    selectedFormat = e.target.getAttribute('data-fmt');
  }
});

function showStatus(msg, type = 'info') {
  statusBox.className = `status-msg ${type}`;
  statusBox.textContent = msg;
  statusBox.style.display = 'block';
}

function updateButtons() {
  const ready = nesstarBytes !== null && ddiXml !== null;
  previewBtn.disabled = !ready;
  convertBtn.disabled = !ready;
  if (ready) {
    showStatus("✓ Files loaded and verified. Ready to preview or convert.", "info");
  }
}

// Load .Nesstar
nesstarInput.addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  nesstarStatus.textContent = `${file.name} (${(file.size / (1024 * 1024)).toFixed(2)} MB)`;
  const buffer = await file.arrayBuffer();
  nesstarBytes = new Uint8Array(buffer);
  updateButtons();
});

// Load DDI XML
ddiInput.addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  ddiStatus.textContent = `${file.name} (${(file.size / 1024).toFixed(1)} KB)`;
  ddiXml = await file.text();
  updateButtons();
});

// Init WASM
async function initWasm() {
  try {
    const wasm = await import('./pkg/nesstar_wasm.js');
    await wasm.default();
    wasmModule = wasm;
    console.log("Nesstar WebAssembly Engine Initialized");
  } catch (err) {
    console.warn("WASM bundle loading via local fallback / worker:", err);
  }
}

initWasm();

// Preview
previewBtn.addEventListener('click', async () => {
  if (!nesstarBytes || !ddiXml) return;
  showStatus("Decoding top 50 rows in WebAssembly...", "info");
  
  try {
    let preview;
    if (wasmModule && wasmModule.preview_nesstar) {
      preview = wasmModule.preview_nesstar(nesstarBytes, ddiXml, 50);
    } else {
      throw new Error("WebAssembly engine is compiling or loading. Please wait a moment.");
    }

    previewMeta.textContent = `Total rows: ${preview.total_rows.toLocaleString()} | Columns: ${preview.total_cols}`;
    
    let html = '<table><thead><tr>';
    preview.headers.forEach(h => html += `<th>${escapeHtml(h)}</th>`);
    html += '</tr></thead><tbody>';

    preview.rows.forEach(row => {
      html += '<tr>';
      row.forEach(cell => html += `<td>${escapeHtml(cell)}</td>`);
      html += '</tr>';
    });
    html += '</tbody></table>';

    tableContainer.innerHTML = html;
    previewCard.style.display = 'block';
    showStatus(`✓ Successfully decoded preview of ${preview.rows.length} rows.`, "success");
  } catch (err) {
    showStatus(`Preview Error: ${err.message || err}`, "error");
  }
});

// Convert & Download
convertBtn.addEventListener('click', async () => {
  if (!nesstarBytes || !ddiXml) return;
  showStatus(`Converting dataset to ${selectedFormat.toUpperCase()} in WebAssembly...`, "info");
  convertBtn.disabled = true;

  try {
    const startTime = performance.now();
    let resultBytes;

    if (wasmModule && wasmModule.convert_nesstar) {
      resultBytes = wasmModule.convert_nesstar(nesstarBytes, ddiXml, selectedFormat);
    } else {
      throw new Error("WebAssembly engine is not ready.");
    }

    const elapsed = ((performance.now() - startTime) / 1000).toFixed(2);
    
    // Create download
    const mimeTypes = {
      csv: 'text/csv;charset=utf-8;',
      tsv: 'text/tab-separated-values;charset=utf-8;',
      jsonl: 'application/x-ndjson;charset=utf-8;',
      json: 'application/json;charset=utf-8;'
    };
    
    const blob = new Blob([resultBytes], { type: mimeTypes[selectedFormat] || 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `survey_export.${selectedFormat}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    showStatus(`✓ Conversion complete in ${elapsed}s! File download triggered (${(blob.size / (1024 * 1024)).toFixed(2)} MB).`, "success");
  } catch (err) {
    showStatus(`Conversion Error: ${err.message || err}`, "error");
  } finally {
    convertBtn.disabled = false;
  }
});

function escapeHtml(str) {
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
