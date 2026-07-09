import type { ContentExtension } from "../types";

/** Stats are not applied to the main panel; keep extension for future meta. */
export const statsExtension: ContentExtension = {
  id: "stats",
  suggest: () => [],
};
