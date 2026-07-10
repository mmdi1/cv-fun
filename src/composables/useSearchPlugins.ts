import { computed, onUnmounted, ref, watch, type Ref } from "vue";
import {
  getState,
  listSearchPluginCatalog,
  loadSearchPluginStates,
  saveSearchPluginStates,
  type SearchPlugin,
  type SearchPluginResult,
  type SearchPluginStates,
} from "../searchPlugins";

export type SearchPluginRow = SearchPlugin & {
  installed: boolean;
  enabled: boolean;
};

/**
 * Optional search-box plugins: install from catalog, match query → content panel.
 */
export function useSearchPlugins(
  query: Ref<string>,
  setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void,
) {
  const states = ref<SearchPluginStates>(loadSearchPluginStates());
  const liveTick = ref(0);
  let liveTimer: ReturnType<typeof setInterval> | undefined;

  const catalog = computed<SearchPluginRow[]>(() =>
    listSearchPluginCatalog().map((p) => {
      const s = getState(states.value, p.id);
      return {
        ...p,
        installed: s.installed,
        enabled: s.installed && s.enabled,
      };
    }),
  );

  const activePlugins = computed(() =>
    catalog.value.filter((p) => p.installed && p.enabled),
  );

  /** First matching installed+enabled search plugin result for current query. */
  const searchPluginResult = computed<SearchPluginResult | null>(() => {
    // depend on liveTick so live plugins refresh
    void liveTick.value;
    const q = query.value;
    for (const p of activePlugins.value) {
      try {
        if (!p.match(q)) continue;
        const r = p.run(q);
        if (r) return r;
      } catch (err) {
        console.warn(`[searchPlugins] ${p.id} failed`, err);
      }
    }
    return null;
  });

  function persist() {
    saveSearchPluginStates(states.value);
  }

  function installPlugin(id: string) {
    const next = { ...states.value };
    next[id] = { installed: true, enabled: true };
    states.value = next;
    persist();
    const name = listSearchPluginCatalog().find((p) => p.id === id)?.name ?? id;
    setStatus(`已安装 · ${name}`, "ok");
  }

  function uninstallPlugin(id: string) {
    const next = { ...states.value };
    next[id] = { installed: false, enabled: false };
    states.value = next;
    persist();
    const name = listSearchPluginCatalog().find((p) => p.id === id)?.name ?? id;
    setStatus(`已卸载 · ${name}`, "ok");
  }

  function toggleSearchPlugin(id: string) {
    const cur = getState(states.value, id);
    if (!cur.installed) {
      installPlugin(id);
      return;
    }
    const next = { ...states.value };
    next[id] = { installed: true, enabled: !cur.enabled };
    states.value = next;
    persist();
    const name = listSearchPluginCatalog().find((p) => p.id === id)?.name ?? id;
    setStatus(next[id]!.enabled ? `已启用 · ${name}` : `已关闭 · ${name}`, "ok");
  }

  // Live refresh (e.g. clock) while a live result is showing
  watch(
    searchPluginResult,
    (r) => {
      clearInterval(liveTimer);
      liveTimer = undefined;
      if (r?.live) {
        liveTimer = setInterval(() => {
          liveTick.value += 1;
        }, 1000);
      }
    },
    { immediate: true },
  );

  onUnmounted(() => {
    clearInterval(liveTimer);
  });

  return {
    searchPluginCatalog: catalog,
    searchPluginResult,
    installSearchPlugin: installPlugin,
    uninstallSearchPlugin: uninstallPlugin,
    toggleSearchPlugin,
  };
}
