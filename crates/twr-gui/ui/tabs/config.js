import { state, set, subscribe } from '../store.js';

const { invoke } = window.__TAURI__.core;

const FIELDS = [
  { key: 'low_percentile',          label: 'Low percentile (0–1)',       type: 'number', step: '0.01' },
  { key: 'activity_threshold',      label: 'Activity threshold',         type: 'number', step: '1'    },
  { key: 'min_days',                label: 'Min days for activity',      type: 'number', step: '1'    },
  { key: 'zero_war_kick_threshold', label: 'Zero-war kick threshold',    type: 'number', step: '1'    },
  { key: 'poor_war_threshold',      label: 'Poor-war threshold',         type: 'number', step: '1'    },
];

export function initConfigTab(panel) {
  panel.innerHTML = `
    <div class="card">
      <h2>Preset</h2>
      <div class="form-row">
        <label>Preset</label>
        <select id="preset-select"><option value="">— none —</option></select>
        <button class="btn ghost" id="preset-save-btn">Save as…</button>
        <button class="btn ghost" id="preset-delete-btn">Delete</button>
      </div>
      <button class="btn ghost" id="restore-defaults-btn">Restore defaults</button>
    </div>
    <div class="card">
      <h2>Thresholds</h2>
      ${FIELDS.map(f => `
        <div class="form-row">
          <label for="cfg-${f.key}">${f.label}</label>
          <input id="cfg-${f.key}" type="${f.type}" step="${f.step}" />
        </div>
      `).join('')}
    </div>
    <div class="card">
      <h2>Output</h2>
      <div class="form-row">
        <label>Formats</label>
        <label class="checkbox-row"><input type="checkbox" data-fmt="xlsx" /> XLSX</label>
        <label class="checkbox-row"><input type="checkbox" data-fmt="csv"  /> CSV</label>
        <label class="checkbox-row"><input type="checkbox" data-fmt="markdown" /> Markdown</label>
      </div>
      <div class="form-row">
        <label for="cfg-reference-time">Reference time (ISO-8601)</label>
        <input id="cfg-reference-time" type="text" placeholder="leave blank for now" style="width:240px" />
      </div>
      <div class="form-row">
        <label for="cfg-output-dir">Output directory</label>
        <input id="cfg-output-dir" type="text" placeholder="default: ./reports/<timestamp>" style="width:300px" />
        <button class="btn ghost" id="pick-output-btn">Browse</button>
      </div>
      <div class="checkbox-row">
        <input type="checkbox" id="cfg-fail-on-warnings" />
        <label for="cfg-fail-on-warnings">Fail on warnings (exit code 3)</label>
      </div>
    </div>
  `;

  // Populate from store once config is available.
  subscribe('config', cfg => { if (cfg) applyConfigToForm(panel, cfg); });
  if (state.config) applyConfigToForm(panel, state.config);

  // Live editing → update store.config
  FIELDS.forEach(f => {
    panel.querySelector(`#cfg-${f.key}`).addEventListener('input', syncFormToStore.bind(null, panel));
  });
  panel.querySelectorAll('input[data-fmt]').forEach(cb => {
    cb.addEventListener('change', syncFormToStore.bind(null, panel));
  });

  // Restore defaults
  panel.querySelector('#restore-defaults-btn').addEventListener('click', async () => {
    const defaults = await invoke('get_default_config');
    set('config', defaults);
  });

  // Preset save
  panel.querySelector('#preset-save-btn').addEventListener('click', async () => {
    const name = prompt('Preset name:');
    if (!name) return;
    await invoke('save_preset', { preset: { name, config: state.config } });
    await loadPresets(panel);
  });

  // Preset delete
  panel.querySelector('#preset-delete-btn').addEventListener('click', async () => {
    const sel = panel.querySelector('#preset-select');
    if (!sel.value) return;
    await invoke('delete_preset', { name: sel.value });
    await loadPresets(panel);
  });

  // Preset load on select change
  panel.querySelector('#preset-select').addEventListener('change', async () => {
    const name = panel.querySelector('#preset-select').value;
    if (!name) return;
    const presets = await invoke('list_presets');
    const found = presets.find(p => p.name === name);
    if (found) set('config', found.config);
  });

  // Output directory picker
  panel.querySelector('#pick-output-btn').addEventListener('click', async () => {
    const dir = await invoke('pick_directory');
    if (dir) panel.querySelector('#cfg-output-dir').value = dir;
  });

  loadPresets(panel);
}

async function loadPresets(panel) {
  const sel = panel.querySelector('#preset-select');
  const prev = sel.value;
  sel.innerHTML = '<option value="">— none —</option>';
  const presets = await invoke('list_presets');
  presets.forEach(p => {
    const opt = document.createElement('option');
    opt.value = p.name;
    opt.textContent = p.name;
    sel.appendChild(opt);
  });
  if (presets.find(p => p.name === prev)) sel.value = prev;
}

function applyConfigToForm(panel, cfg) {
  FIELDS.forEach(f => {
    const el = panel.querySelector(`#cfg-${f.key}`);
    if (el) el.value = cfg[f.key] ?? '';
  });
  const fmts = new Set(cfg.formats || []);
  panel.querySelectorAll('input[data-fmt]').forEach(cb => {
    cb.checked = fmts.has(cb.dataset.fmt);
  });
}

function syncFormToStore(panel) {
  const cfg = { ...state.config };
  FIELDS.forEach(f => {
    const val = panel.querySelector(`#cfg-${f.key}`).value;
    cfg[f.key] = f.type === 'number' ? parseFloat(val) : val;
  });
  cfg.formats = [];
  panel.querySelectorAll('input[data-fmt]').forEach(cb => {
    if (cb.checked) cfg.formats.push(cb.dataset.fmt);
  });
  set('config', cfg);
}

export function getRunOptions(panel) {
  return {
    reference_time: panel?.querySelector('#cfg-reference-time')?.value || null,
    output: panel?.querySelector('#cfg-output-dir')?.value || null,
    fail_on_warnings: panel?.querySelector('#cfg-fail-on-warnings')?.checked || false,
  };
}
