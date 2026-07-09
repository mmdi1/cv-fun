/**
 * Content extension system
 * ------------------------
 * - Registry: `extensionRegistry` holds all plugins
 * - Strategy: each extension may `suggest()` parse options
 * - Pipeline: `resolveContent()` returns original panel + suggestions
 *
 * Main panel always shows **original** content.
 * User clicks a suggestion row item to apply its body into the panel.
 *
 * How to add an extension:
 * 1. Create `src/extensions/builtins/my-ext.ts` implementing `ContentExtension`
 * 2. Register below in `bootstrapExtensions`
 */

import { base64Extension } from "./builtins/base64";
import { colorExtension } from "./builtins/color";
import { imageExtension } from "./builtins/image";
import { jsonExtension } from "./builtins/json";
import { sqlExtension } from "./builtins/sql";
import { statsExtension } from "./builtins/stats";
import { timestampExtension } from "./builtins/timestamp";
import { urlExtension } from "./builtins/url";
import { uuidExtension } from "./builtins/uuid";
import { wordExtension } from "./builtins/word";
import { extensionRegistry } from "./registry";

let bootstrapped = false;

export function bootstrapExtensions(): void {
  if (bootstrapped) return;
  bootstrapped = true;
  extensionRegistry.registerAll([
    jsonExtension,
    sqlExtension,
    imageExtension,
    timestampExtension,
    urlExtension,
    uuidExtension,
    colorExtension,
    base64Extension,
    wordExtension,
    statsExtension,
  ]);
}

bootstrapExtensions();

export { extensionRegistry } from "./registry";
export { resolveContent } from "./pipeline";
export type { ContentExtension, ContentContext, ResolveResult } from "./types";
export { recoverJson, inferJsonOutput, unescapeJsonNoise } from "./builtins/json";
