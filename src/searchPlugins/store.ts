import { listSearchPluginCatalog } from "./registry";
import type { SearchPluginState, SearchPluginStates } from "./types";

const STORAGE_KEY = "nfun-cv.searchPlugins.v1";

function defaults(): SearchPluginStates {
  const out: SearchPluginStates = {};
  for (const p of listSearchPluginCatalog()) {
    out[p.id] = {
      installed: p.defaultInstalled,
      enabled: p.defaultInstalled,
    };
  }
  return out;
}

export function loadSearchPluginStates(): SearchPluginStates {
  const base = defaults();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return base;
    const parsed = JSON.parse(raw) as SearchPluginStates;
    if (!parsed || typeof parsed !== "object") return base;
    for (const id of Object.keys(base)) {
      const s = parsed[id];
      if (s && typeof s === "object") {
        base[id] = {
          installed: !!s.installed,
          enabled: s.installed ? !!s.enabled : false,
        };
      }
    }
  } catch {
    // ignore corrupt storage
  }
  return base;
}

export function saveSearchPluginStates(states: SearchPluginStates): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(states));
  } catch {
    // quota / private mode
  }
}

export function getState(
  states: SearchPluginStates,
  id: string,
): SearchPluginState {
  return states[id] ?? { installed: false, enabled: false };
}
