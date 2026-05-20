import { state, set, subscribe } from '../store.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export function initRunTab(panel) {
  panel.innerHTML = `
    <div class="card">
      <h2>Actions</h2>
      <div style="display:flex;gap:8px;align-items:center">
        <button class="btn primary" id="run-btn" disabled>Run Analysis</button>
        <button class="btn ghost" id="validate-btn" disabled>Validate Only</button>
        <button class="btn ghost" id="cancel-btn" disabled>Cancel</button>
      </div>
      <p id="run-status" style="margin-top:8px;font-size:0.875rem;color:var(--muted)"></p>
    </div>
    <div class="card" id="progress-card" style="display:none">
      <h2>Progress</h2>
      <p id="progress-stage" style="font-size:0.875rem;margin-bottom:6px"></p>
      <div class="progress-bar-wrap"><div class="progress-bar-fill" id="progress-fill"></div></div>
      <div id="warning-section" style="margin-top:12px;display:none">
        <span class="warning-chip" id="warning-chip">0 warnings</span>
        <div class="warning-list" id="warning-list"></div>
      </div>
    </div>
    <div class="card" id="output-card" style="display:none">
      <h2>Output</h2>
      <div id="output-paths"></div>
      <div style="margin-top:12px;display:flex;gap:8px">
        <button class="btn ghost" id="open-folder-btn">Open folder</button>
        <button class="btn ghost" id="open-md-btn" style="display:none">Open summary.md</button>
      </div>
    </div>
  `;

  subscribe('warPaths', updateRunBtns.bind(null, panel));
  subscribe('activityPath', updateRunBtns.bind(null, panel));

  panel.querySelector('#run-btn').addEventListener('click', () => startRun(panel, 'analyze'));
  panel.querySelector('#validate-btn').addEventListener('click', () => startRun(panel, 'validate'));
  panel.querySelector('#cancel-btn').addEventListener('click', async () => {
    if (state.currentRunId) {
      await invoke('cancel_run', { runId: state.currentRunId });
      set('currentRunId', null);
      setStatus(panel, 'Cancelled.', 'var(--warn)');
      setRunning(panel, false);
    }
  });

  panel.querySelector('#warning-chip').addEventListener('click', () => {
    panel.querySelector('#warning-list').classList.toggle('open');
  });
}

function updateRunBtns(panel) {
  const ready = state.warPaths.length > 0 && !!state.activityPath;
  panel.querySelector('#run-btn').disabled = !ready;
  panel.querySelector('#validate-btn').disabled = !ready;
}

function setStatus(panel, msg, color) {
  const el = panel.querySelector('#run-status');
  el.textContent = msg;
  el.style.color = color || 'var(--muted)';
}

function setRunning(panel, running) {
  panel.querySelector('#run-btn').disabled = running || !(state.warPaths.length > 0 && !!state.activityPath);
  panel.querySelector('#validate-btn').disabled = running || !(state.warPaths.length > 0 && !!state.activityPath);
  panel.querySelector('#cancel-btn').disabled = !running;
}

async function startRun(panel, mode) {
  set('warnings', []);
  panel.querySelector('#progress-card').style.display = '';
  panel.querySelector('#output-card').style.display = 'none';
  panel.querySelector('#warning-section').style.display = 'none';
  panel.querySelector('#warning-list').innerHTML = '';
  panel.querySelector('#warning-list').classList.remove('open');
  panel.querySelector('#progress-fill').style.width = '0%';

  setRunning(panel, true);
  setStatus(panel, 'Starting…', 'var(--muted)');

  const cfg = state.config || {};
  let runHandle;
  try {
    if (mode === 'analyze') {
      runHandle = await invoke('spawn_analyze', {
        args: {
          wars: state.warPaths,
          activity: state.activityPath,
          output: null,
          formats: cfg.formats || null,
          reference_time: null,
          low_percentile: cfg.low_percentile ?? null,
          activity_threshold: cfg.activity_threshold ?? null,
          min_days: cfg.min_days ?? null,
          zero_war_kick_threshold: cfg.zero_war_kick_threshold ?? null,
          poor_war_threshold: cfg.poor_war_threshold ?? null,
          fail_on_warnings: false,
          config_path: null,
        },
      });
    } else {
      runHandle = await invoke('spawn_validate', {
        args: {
          wars: state.warPaths,
          activity: state.activityPath,
          fail_on_warnings: false,
        },
      });
    }
  } catch (e) {
    setStatus(panel, `Error: ${e}`, 'var(--error)');
    setRunning(panel, false);
    return;
  }

  set('currentRunId', runHandle.run_id);

  // Progress stages → % mapping (5 stages)
  const STAGE_PCT = { expand_wars: 10, parse_war: 50, parse_activity: 65, analyze: 80, write: 95 };
  let warTotal = 0;
  let warDone = 0;

  const unlisten = await listen(`cli://event/${runHandle.run_id}`, ev => {
    const event = ev.payload;
    if (!event) return;

    if (event.type === 'progress') {
      const stage = event.stage;
      let pct = STAGE_PCT[stage] || 0;

      if (stage === 'parse_war') {
        warTotal = event.total || warTotal;
        warDone = event.current || warDone;
        pct = 10 + Math.round((warDone / (warTotal || 1)) * 40);
      }

      panel.querySelector('#progress-fill').style.width = `${pct}%`;
      panel.querySelector('#progress-stage').textContent = stageLabel(event);
    }

    if (event.type === 'warning') {
      const warns = [...state.warnings, event];
      set('warnings', warns);
      renderWarnings(panel, warns);
    }

    if (event.type === 'done' || event.type === 'validate_done') {
      const exitCode = event.exit_code;
      panel.querySelector('#progress-fill').style.width = '100%';
      setRunning(panel, false);
      set('currentRunId', null);
      unlisten();

      if (event.type === 'done') {
        renderOutputs(panel, event);
        panel.querySelector('#output-card').style.display = '';
        setStatus(panel, exitCode === 0 ? 'Done.' : `Done (exit ${exitCode}).`,
          exitCode === 0 ? 'var(--success)' : 'var(--warn)');
      } else {
        setStatus(panel, `Validation complete — ${event.warning_count} warning(s).`,
          event.warning_count ? 'var(--warn)' : 'var(--success)');
      }
    }
  });
}

function stageLabel(event) {
  switch (event.stage) {
    case 'expand_wars':   return `Expanding war files: ${event.detail || ''}`;
    case 'parse_war':     return `Parsing war ${event.current}/${event.total}: ${fileBasename(event.file)}`;
    case 'parse_activity':return `Parsing activity: ${fileBasename(event.file)}`;
    case 'analyze':       return 'Running analysis…';
    case 'write':         return `Writing ${event.format}: ${fileBasename(event.path)}`;
    default:              return event.stage || '';
  }
}

function fileBasename(path) {
  return (path || '').split(/[\\/]/).pop();
}

function renderWarnings(panel, warns) {
  const section = panel.querySelector('#warning-section');
  section.style.display = warns.length ? '' : 'none';
  const chip = panel.querySelector('#warning-chip');
  chip.textContent = `${warns.length} warning${warns.length !== 1 ? 's' : ''}`;

  const list = panel.querySelector('#warning-list');
  list.innerHTML = warns.map(w => `
    <div class="warning-item">
      <span class="kind">${w.kind}</span>
      ${w.context ? ` <span style="color:var(--muted)">(${w.context})</span>` : ''}
      — ${w.message}
      <br><small style="color:var(--muted)">${w.source}</small>
    </div>
  `).join('');
}

function renderOutputs(panel, event) {
  const outputs = event.outputs || {};
  const pathsDiv = panel.querySelector('#output-paths');
  let html = '';

  if (outputs.xlsx) {
    html += pathRow(outputs.xlsx, 'XLSX');
  }
  (outputs.csv || []).forEach(p => { html += pathRow(p, 'CSV'); });
  if (outputs.markdown) {
    html += pathRow(outputs.markdown, 'Markdown');
    const btn = panel.querySelector('#open-md-btn');
    btn.style.display = '';
    btn.onclick = () => invoke('open_path', { path: outputs.markdown });
  }
  pathsDiv.innerHTML = html;

  const folderBtn = panel.querySelector('#open-folder-btn');
  folderBtn.onclick = () => {
    if (event.output_dir) invoke('open_path', { path: event.output_dir });
  };
}

function pathRow(path, label) {
  return `<div class="output-path"><b>${label}</b>: <span>${path}</span></div>`;
}
