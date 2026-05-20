import { state, set, subscribe } from '../store.js';

const { invoke } = window.__TAURI__.core;
const { getCurrentWebview } = window.__TAURI__.webview;

// ISO-8601 date embedded in Torn war filenames: _2026-01-01T00-00-00Z
const WAR_DATE_RE = /_(\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z)\.csv$/i;

function validateWarFile(path) {
  const match = WAR_DATE_RE.exec(path.replace(/\\/g, '/').split('/').pop());
  if (!match) return { ok: false, label: 'no date' };
  const iso = match[1].replace(/-(\d{2})-(\d{2}Z)$/, 'T$1:$2').replace('T', 'T').replace(/T(\d{2})-(\d{2})-(\d{2}Z)/, 'T$1:$2:$3');
  return { ok: true, label: match[1] };
}

export function initInputsTab(panel) {
  panel.innerHTML = `
    <div class="card">
      <h2>War CSV files</h2>
      <div id="war-drop" class="drop-zone">
        Drop war CSV files here, or <button class="btn ghost" id="war-pick-btn">browse</button>
      </div>
      <ul id="war-list" class="file-list"></ul>
    </div>
    <div class="card">
      <h2>Member Activity CSV</h2>
      <div id="activity-drop" class="drop-zone">
        Drop activity CSV here, or <button class="btn ghost" id="activity-pick-btn">browse</button>
      </div>
      <p id="activity-path" style="font-size:0.8rem;margin-top:8px;color:var(--muted)"></p>
    </div>
  `;

  renderWarList(panel);
  subscribe('warPaths', () => renderWarList(panel));
  subscribe('activityPath', path => {
    panel.querySelector('#activity-path').textContent = path || '';
    const drop = panel.querySelector('#activity-drop');
    drop.classList.toggle('has-files', !!path);
  });

  // War file picker
  panel.querySelector('#war-pick-btn').addEventListener('click', async () => {
    const files = await invoke('pick_files', { multi: true });
    if (files && files.length) addWarPaths(files);
  });

  // Activity file picker
  panel.querySelector('#activity-pick-btn').addEventListener('click', async () => {
    const files = await invoke('pick_files', { multi: false });
    if (files && files.length) set('activityPath', files[0]);
  });

  // Drag-and-drop via Tauri webview events
  getCurrentWebview().onDragDropEvent(event => {
    if (event.payload.type === 'drop') {
      const paths = event.payload.paths || [];
      const csvs = paths.filter(p => p.toLowerCase().endsWith('.csv'));
      if (!csvs.length) return;

      // Heuristic: files with a war date pattern → war list; otherwise activity
      const wars = csvs.filter(p => WAR_DATE_RE.test(p.split(/[\\/]/).pop()));
      const others = csvs.filter(p => !WAR_DATE_RE.test(p.split(/[\\/]/).pop()));
      if (wars.length) addWarPaths(wars);
      if (others.length) set('activityPath', others[0]);
    }
  });

  // Visual drag feedback
  ['war-drop', 'activity-drop'].forEach(id => {
    const el = panel.querySelector(`#${id}`);
    el.addEventListener('dragover', e => { e.preventDefault(); el.classList.add('drag-over'); });
    el.addEventListener('dragleave', () => el.classList.remove('drag-over'));
    el.addEventListener('drop', () => el.classList.remove('drag-over'));
  });
}

function addWarPaths(paths) {
  const existing = new Set(state.warPaths);
  const added = paths.filter(p => !existing.has(p));
  set('warPaths', [...state.warPaths, ...added]);
}

function renderWarList(panel) {
  const ul = panel.querySelector('#war-list');
  if (!ul) return;
  ul.innerHTML = '';
  const drop = panel.querySelector('#war-drop');
  if (drop) drop.classList.toggle('has-files', state.warPaths.length > 0);

  state.warPaths.forEach((path, idx) => {
    const validation = validateWarFile(path);
    const li = document.createElement('li');
    li.innerHTML = `
      <span class="badge ${validation.ok ? 'ok' : 'err'}">${validation.label}</span>
      <span title="${path}">${path.split(/[\\/]/).pop()}</span>
      <button class="remove-btn" data-idx="${idx}" title="Remove">×</button>
    `;
    ul.appendChild(li);
  });

  ul.querySelectorAll('.remove-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const idx = parseInt(btn.dataset.idx);
      const newPaths = state.warPaths.filter((_, i) => i !== idx);
      set('warPaths', newPaths);
    });
  });
}
