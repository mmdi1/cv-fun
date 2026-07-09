import type { ContentExtension } from "./types";

/**
 * Registry pattern — single place to register/unregister content extensions.
 * Extensions are sorted by id for stable iteration; display priority is per-claim.
 */
class ExtensionRegistry {
  private map = new Map<string, ContentExtension>();

  register(ext: ContentExtension): void {
    if (this.map.has(ext.id)) {
      console.warn(`[extensions] overriding extension "${ext.id}"`);
    }
    this.map.set(ext.id, ext);
  }

  registerAll(exts: ContentExtension[]): void {
    for (const e of exts) this.register(e);
  }

  unregister(id: string): boolean {
    return this.map.delete(id);
  }

  get(id: string): ContentExtension | undefined {
    return this.map.get(id);
  }

  list(): ContentExtension[] {
    return [...this.map.values()];
  }

  clear(): void {
    this.map.clear();
  }
}

/** App-wide singleton registry */
export const extensionRegistry = new ExtensionRegistry();
