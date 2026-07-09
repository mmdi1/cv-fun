import type { ContentExtension } from "../types";

/** Word detection kept as no-op suggestions (metadata only if needed later). */
export const wordExtension: ContentExtension = {
  id: "word",
  suggest: () => [],
};
