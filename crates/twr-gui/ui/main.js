import { initInputsTab }  from './tabs/inputs.js';
import { initConfigTab }  from './tabs/config.js';
import { initRunTab }     from './tabs/run.js';
import { initHistoryTab } from './tabs/history.js';
import { set }            from './store.js';

const { invoke } = window.__TAURI__.core;

// Tab switching
document.querySelectorAll('.tab-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    const tabId = btn.dataset.tab;
    document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.tab-panel').forEach(p => p.classList.remove('active'));
    btn.classList.add('active');
    document.getElementById(`tab-${tabId}`).classList.add('active');
  });
});

// Boot
(async () => {
  try {
    const defaultCfg = await invoke('get_default_config');
    set('config', defaultCfg);
  } catch (e) {
    console.error('get_default_config failed', e);
  }

  initInputsTab(document.getElementById('tab-inputs'));
  initConfigTab(document.getElementById('tab-config'));
  initRunTab(document.getElementById('tab-run'));
  initHistoryTab(document.getElementById('tab-history'));
})();
