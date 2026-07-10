/** Result shown in the right content panel when a search plugin matches. */
export type SearchPluginResult = {
  pluginId: string;
  /** Head label, e.g. "时间" */
  title: string;
  /** Full body for the content area */
  body: string;
  /** Text written to clipboard (defaults to body) */
  copyText?: string;
  /** Optional hint chip */
  hint?: string;
  /** When true, host may re-run on an interval (live clock) */
  live?: boolean;
};

/**
 * Search-box plugin: match typed query → panel result.
 * Separate from clipboard content plugins (Rust/user scripts).
 */
export type SearchPlugin = {
  id: string;
  name: string;
  description: string;
  version: string;
  /** Example trigger words for UI help */
  triggers: string[];
  /** If false, user must install from catalog (default off). */
  defaultInstalled: boolean;
  /** Whether query should activate this plugin (already installed+enabled checked by host). */
  match(query: string): boolean;
  run(query: string): SearchPluginResult | null;
};

export type SearchPluginState = {
  installed: boolean;
  enabled: boolean;
};

export type SearchPluginStates = Record<string, SearchPluginState>;
