import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import type { EcdictStatus, PluginInfo } from "../core/types";
import type { SamplePack } from "../api";

export type PluginsTab = "list" | "custom";

export function usePlugins(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const pluginsOpen = ref(false);
  const pluginsTab = ref<PluginsTab>("list");
  const plugins = ref<PluginInfo[]>([]);
  const pluginsLoading = ref(false);
  const ecdict = ref<EcdictStatus | null>(null);
  const ecdictBusy = ref(false);
  const ecdictProgress = ref("");
  const protocolHelp = ref("");
  const samples = ref<SamplePack[]>([]);
  const samplesLoading = ref(false);

  async function refreshPlugins() {
    pluginsLoading.value = true;
    try {
      plugins.value = (await api.listPlugins()) || [];
      ecdict.value = await api.ecdictStatus();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    } finally {
      pluginsLoading.value = false;
    }
  }

  async function loadCustomTab() {
    samplesLoading.value = true;
    try {
      if (!protocolHelp.value) {
        protocolHelp.value = await api.pluginProtocolHelp();
      }
      samples.value = (await api.listPluginSamples()) || [];
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    } finally {
      samplesLoading.value = false;
    }
  }

  async function openPlugins() {
    pluginsOpen.value = true;
    pluginsTab.value = "list";
    await refreshPlugins();
  }

  async function switchTab(tab: PluginsTab) {
    pluginsTab.value = tab;
    if (tab === "list") {
      await refreshPlugins();
    } else {
      await loadCustomTab();
    }
  }

  async function togglePlugin(p: PluginInfo) {
    try {
      await api.setPluginEnabled(p.id, !p.enabled);
      p.enabled = !p.enabled;
      setStatus(p.enabled ? `已启用 · ${p.name}` : `已关闭 · ${p.name}`, "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function importPluginDir() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择插件目录（含 plugin.json）",
      });
      if (!selected || Array.isArray(selected)) return;
      const info = await api.importPlugin(selected);
      setStatus(`已导入 · ${info.name}`, "ok");
      pluginsTab.value = "list";
      await refreshPlugins();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function removeUserPlugin(p: PluginInfo) {
    if (p.builtin) return;
    try {
      await api.removePlugin(p.id);
      setStatus(`已删除 · ${p.name}`, "ok");
      await refreshPlugins();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function downloadEcdict() {
    if (ecdictBusy.value) return;
    ecdictBusy.value = true;
    ecdictProgress.value = "准备下载…";
    try {
      const status = await api.installEcdict();
      ecdict.value = status;
      setStatus(`ECDICT 就绪 · ${status.entries} 词条`, "ok");
      ecdictProgress.value = "";
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
      ecdictProgress.value = "";
    } finally {
      ecdictBusy.value = false;
    }
  }

  async function pickExportDir(title: string): Promise<string | null> {
    const selected = await open({
      directory: true,
      multiple: false,
      title,
    });
    if (!selected || Array.isArray(selected)) return null;
    return selected;
  }

  async function exportSample(id: string, name: string) {
    try {
      const dest = await pickExportDir(`导出示例「${name}」到…`);
      if (!dest) return;
      const path = await api.exportPluginSample(id, dest);
      setStatus(`已导出 · ${path}`, "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function exportAllSamples() {
    try {
      const dest = await pickExportDir("导出全部示例到…");
      if (!dest) return;
      const paths = await api.exportAllPluginSamples(dest);
      setStatus(`已导出 ${paths.length} 个示例`, "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  return {
    pluginsOpen,
    pluginsTab,
    plugins,
    pluginsLoading,
    ecdict,
    ecdictBusy,
    ecdictProgress,
    protocolHelp,
    samples,
    samplesLoading,
    openPlugins,
    switchTab,
    refreshPlugins,
    togglePlugin,
    importPluginDir,
    removeUserPlugin,
    downloadEcdict,
    exportSample,
    exportAllSamples,
  };
}
