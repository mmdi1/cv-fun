import type { ContentExtension } from "../types";

/** Image has no text suggestions; main panel handles image mode from original. */
export const imageExtension: ContentExtension = {
  id: "image",
  suggest: () => [],
};
