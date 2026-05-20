import { state, set } from '../store.js';

const { invoke } = window.__TAURI__.core;

export function initHistoryTab(panel) {
  panel.innerHTML = `
    <div class="card">
      <h2>Reports root</h2>
      <div class="form-row">
        <label>Directory</label>
        <input id="reports-root-input" type="text" placeholder="./reports" style="width:340px" />
        <button class="btn ghost" id="pick-root-btn">Browse</button>
        <button class="btn ghost" id="refresh-btn">Refresh</button>
      </div>
    </div>
    <div class="card">
      <h2>Run history</h2>
      <table class="history-table">
        <thead>
          <tr>
            <th>Date</th>
            <th>Auto-kick</th>
            <th>Poor-war</th>
            <th>Warnings</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody id="history-tbody"></tbody>
      </table>
    </div>
  `;

  // Load persisted reports root
  invoke('get_settings').then(settings => {
    if (settings.reports_root) {
      panel.querySelector('#reports-root-input').value = settings.reports_root;
    }
    loadHistory(panel);
  });

  panel.querySelector('#pick-root-btn').addEventListener('click', async () => {
    const dir = await invoke('pick_directory');
    if (dir) {
      panel.querySelector('#reports-root-input').value = dir;
      await invoke('set_settings', { settings: { reports_root: dir } });
      await loadHistory(panel);
    }
  });

  panel.querySelector('#refresh-btn').addEventListener('click', () => loadHistory(panel));
}

async function loadHistory(panel) {
  const root = panel.querySelector('#reports-root-input').value || './reports';
  let runs;
  try {
    runs = await invoke('list_history', { reportsRoot: root });
  } catch (e) {
    panel.querySelector('#history-tbody').innerHTML =
      `<tr><td colspan="5" style="color:var(--muted);padding:16px">${e}</td></tr>`;
    return;
  }

  const tbody = panel.querySelector('#history-tbody');
  if (!runs || runs.length === 0) {
    tbody.innerHTML = `<tr><td colspan="5" style="color:var(--muted);padding:16px">No runs found in ${root}</td></tr>`;
    return;
  }

  tbody.innerHTML = runs.map(run => {
    const date = run.reference_time ? new Date(run.reference_time).toLocaleString() : run.run_id;
    const autoKick = run.list_sizes?.auto_kick ?? '—';
    const poorWar = run.list_sizes?.poor_war ?? '—';
    return `
      <tr>
        <td>${date}</td>
        <td>${autoKick}</td>
        <td>${poorWar}</td>
        <td>${run.warning_count}</td>
        <td>
          <button class="btn ghost open-folder" data-dir="${escapeAttr(run.output_dir)}" style="margin-right:4px">Open</button>
          <button class="btn ghost rerun" data-idx="${runs.indexOf(run)}">Re-run</button>
        </td>
      </tr>
    `;
  }).join('');

  tbody.querySelectorAll('.open-folder').forEach(btn => {
    btn.addEventListener('click', () => invoke('open_path', { path: btn.dataset.dir }));
  });

  tbody.querySelectorAll('.rerun').forEach(btn => {
    btn.addEventListener('click', () => {
      const run = runs[parseInt(btn.dataset.idx)];
      rerunFromHistory(run);
    });
  });
}

function rerunFromHistory(run) {
  // Repopulate inputs + config store from run.json data.
  if (run.input_war_files && run.input_war_files.length) {
    // Dynamic import to avoid circular deps
    import('../store.js').then(({ set }) => {
      set('warPaths', run.input_war_files);
      if (run.input_activity_file) set('activityPath', run.input_activity_file);
      if (run.config) set('config', run.config);
    });
    // Switch to inputs tab
    const inputsBtn = document.querySelector('.tab-btn[data-tab="inputs"]');
    if (inputsBtn) inputsBtn.click();
  }
}

function escapeAttr(str) {
  return (str || '').replace(/"/g, '&quot;');
}
