import { invoke } from "@tauri-apps/api/core";
import type {
  EntrySummary,
  ListItemView,
  Segment,
  SyncReport,
  SyncSettingsView,
  WordList,
} from "./types";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) {
    throw new Error(
      "Pistachio Dictionary is a desktop app. Start it with `npm run tauri dev` — the plain browser preview has no dictionary backend."
    );
  }
  return invoke<T>(cmd, args);
}

export const api = {
  // dictionary
  search: (query: string) => call<EntrySummary[]>("search_entries", { query, limit: 60 }),
  getEntry: (id: number) => call<EntrySummary>("get_entry", { id }),
  segmentText: (text: string) => call<Segment[]>("segment_text", { text }),

  // word lists
  getLists: () => call<WordList[]>("get_lists"),
  createList: (name: string) => call<WordList>("create_list", { name }),
  renameList: (id: string, name: string) => call<void>("rename_list", { id, name }),
  deleteList: (id: string) => call<void>("delete_list", { id }),
  getListItems: (listId: string) => call<ListItemView[]>("get_list_items", { listId }),
  addToList: (listId: string, entryId: number) =>
    call<ListItemView>("add_to_list", { listId, entryId }),
  removeItem: (itemId: string) => call<void>("remove_item", { itemId }),
  moveItem: (itemId: string, newIndex: number) =>
    call<void>("move_item", { itemId, newIndex }),
  exportListTsv: (listId: string, path: string) =>
    call<void>("export_list_tsv", { listId, path }),

  // sync
  syncNow: () => call<SyncReport>("sync_now"),
  getSyncStatus: () => call<SyncSettingsView>("get_sync_status"),
  setSyncSettings: (serverUrl: string, authToken: string) =>
    call<void>("set_sync_settings", { serverUrl, authToken }),
};
