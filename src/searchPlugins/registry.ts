import { timeSearchPlugin } from "./builtins/time";
import type { SearchPlugin } from "./types";

/** Built-in catalog of search plugins (install optional unless defaultInstalled). */
const CATALOG: SearchPlugin[] = [timeSearchPlugin];

export function listSearchPluginCatalog(): SearchPlugin[] {
  return CATALOG.slice();
}

export function getSearchPlugin(id: string): SearchPlugin | undefined {
  return CATALOG.find((p) => p.id === id);
}
