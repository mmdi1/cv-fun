import { createApp } from "vue";
import "./styles.css";

async function boot() {
  let label = "main";
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    label = getCurrentWindow().label;
  } catch {
    /* browser preview */
  }

  // Paste panel: window label and/or URL hash (#paste) from WebviewUrl.
  const hash = (location.hash || "").replace(/^#\/?/, "");
  const isPaste = label === "paste" || hash === "paste" || hash.startsWith("paste");

  if (isPaste) {
    const { default: PastePopup } = await import("./PastePopup.vue");
    createApp(PastePopup).mount("#app");
    return;
  }

  const { default: App } = await import("./App.vue");
  createApp(App).mount("#app");
}

void boot();
