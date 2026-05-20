// Central app state — imported by all tabs.

export const state = {
  warPaths: [],       // array of absolute path strings
  activityPath: null, // string or null
  config: null,       // PresetConfig object (loaded from get_default_config)
  currentRunId: null,
  warnings: [],       // CliEvent Warning objects from the current run
};

// Listeners keyed by field name.
const listeners = {};

export function subscribe(field, fn) {
  if (!listeners[field]) listeners[field] = [];
  listeners[field].push(fn);
}

export function set(field, value) {
  state[field] = value;
  (listeners[field] || []).forEach(fn => fn(value));
}
