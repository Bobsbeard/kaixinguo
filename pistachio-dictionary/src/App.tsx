import { useCallback, useEffect, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, inTauri } from "./api";
import type {
  EntrySummary,
  ListItemView,
  Segment,
  SyncSettingsView,
  WordList,
} from "./types";
import SearchBar from "./components/SearchBar";
import EntryView from "./components/EntryView";
import ListSidebar from "./components/ListSidebar";
import ListView from "./components/ListView";
import SyncBar from "./components/SyncBar";
import SettingsModal from "./components/SettingsModal";
import { autoUpdate, restartApp, type UpdatePhase } from "./updater";

function errMsg(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export default function App() {
  const [lists, setLists] = useState<WordList[]>([]);
  const [activeListId, setActiveListId] = useState<string | null>(null);
  const [listItems, setListItems] = useState<ListItemView[]>([]);
  const [query, setQuery] = useState("");
  const [textMode, setTextMode] = useState(false);
  const [results, setResults] = useState<EntrySummary[]>([]);
  const [segments, setSegments] = useState<Segment[] | null>(null);
  const [selected, setSelected] = useState<EntrySummary | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncSettingsView | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [updatePhase, setUpdatePhase] = useState<UpdatePhase>("idle");
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const showToast = useCallback((msg: string) => {
    window.clearTimeout(toastTimer.current);
    setToast(msg);
    toastTimer.current = window.setTimeout(() => setToast(null), 4000);
  }, []);

  const refreshLists = useCallback(async () => {
    try {
      setLists(await api.getLists());
    } catch (e) {
      showToast(errMsg(e));
    }
  }, [showToast]);

  const refreshItems = useCallback(
    async (listId: string) => {
      try {
        setListItems(await api.getListItems(listId));
      } catch (e) {
        showToast(errMsg(e));
      }
    },
    [showToast]
  );

  const refreshSync = useCallback(async () => {
    try {
      setSyncStatus(await api.getSyncStatus());
    } catch {
      /* sync status is best-effort */
    }
  }, []);

  // Initial load + periodic sync-status polling (FR-15).
  useEffect(() => {
    refreshLists();
    refreshSync();
    const t = window.setInterval(refreshSync, 5000);
    return () => window.clearInterval(t);
  }, [refreshLists, refreshSync]);

  // Silent auto-update check on startup (GitHub Releases via Tauri updater).
  useEffect(() => {
    autoUpdate(setUpdatePhase);
  }, []);

  // Debounced search / segmentation (FR-2, FR-3, FR-5).
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults([]);
      setSegments(null);
      return;
    }
    const t = window.setTimeout(async () => {
      try {
        if (textMode && /[㐀-鿿豈-﫿]/.test(q)) {
          setSegments(await api.segmentText(q));
          setResults([]);
        } else {
          setResults(await api.search(q));
          setSegments(null);
        }
      } catch (e) {
        showToast(errMsg(e));
      }
    }, 150);
    return () => window.clearTimeout(t);
  }, [query, textMode, showToast]);

  const activeList = lists.find((l) => l.id === activeListId) ?? null;
  const searching = query.trim().length > 0;

  const handleSelectList = (id: string | null) => {
    setActiveListId(id);
    setQuery("");
    if (id) refreshItems(id);
  };

  const handleCreateList = async (name: string) => {
    try {
      const list = await api.createList(name);
      await refreshLists();
      refreshSync();
      handleSelectList(list.id);
      showToast(`List “${name}” created`);
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleRenameList = async (id: string, name: string) => {
    try {
      await api.renameList(id, name);
      refreshLists();
      refreshSync();
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleDeleteList = async (id: string) => {
    try {
      await api.deleteList(id);
      if (activeListId === id) {
        setActiveListId(null);
        setListItems([]);
      }
      refreshLists();
      refreshSync();
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleAddToList = async (entryId: number, listId: string) => {
    try {
      await api.addToList(listId, entryId);
      showToast("Added to list ✓");
      refreshLists();
      refreshSync();
      if (activeListId === listId) refreshItems(listId);
    } catch (e) {
      const msg = errMsg(e);
      // FR-9: duplicates prompt instead of silently duplicating.
      showToast(msg.includes("duplicate:") ? msg.replace("duplicate: ", "") : msg);
    }
  };

  const handleMove = async (itemId: string, newIndex: number) => {
    try {
      await api.moveItem(itemId, newIndex);
      if (activeListId) refreshItems(activeListId);
      refreshSync();
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleRemove = async (itemId: string) => {
    try {
      await api.removeItem(itemId);
      if (activeListId) refreshItems(activeListId);
      refreshLists();
      refreshSync();
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleExport = async () => {
    if (!activeList) return;
    try {
      const path = await save({
        defaultPath: `${activeList.name}.tsv`,
        filters: [{ name: "TSV", extensions: ["tsv"] }],
      });
      if (!path) return;
      await api.exportListTsv(activeList.id, path);
      showToast(`Exported to ${path}`);
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  const handleSyncNow = async () => {
    setSyncing(true);
    try {
      const report = await api.syncNow();
      showToast(report.message);
      refreshLists();
      if (activeListId) refreshItems(activeListId);
    } catch (e) {
      showToast(errMsg(e));
    } finally {
      setSyncing(false);
      refreshSync();
    }
  };

  const handleSaveSettings = async (url: string, token: string) => {
    try {
      await api.setSyncSettings(url, token);
      setSettingsOpen(false);
      refreshSync();
      showToast("Sync settings saved");
    } catch (e) {
      showToast(errMsg(e));
    }
  };

  return (
    <div className="flex h-full flex-col bg-white text-stone-800">
      <header className="flex items-center justify-between border-b border-stone-200 px-4 py-2.5">
        <div className="flex items-baseline gap-2">
          <span className="text-lg font-bold text-pistachio-700">开心果词典</span>
          <span className="text-sm text-stone-400">Pistachio Dictionary</span>
          <span className="rounded bg-pistachio-100 px-1.5 py-0.5 text-[10px] font-medium text-pistachio-700">
            offline · CC-CEDICT
          </span>
        </div>
        <SyncBar
          status={syncStatus}
          syncing={syncing}
          onSyncNow={handleSyncNow}
          onOpenSettings={() => setSettingsOpen(true)}
        />
      </header>

      {!inTauri && (
        <div className="bg-amber-50 px-4 py-2 text-sm text-amber-800">
          Browser preview only — run <code className="font-mono">npm run tauri dev</code> for the
          real desktop app with the offline dictionary backend.
        </div>
      )}

      {updatePhase !== "idle" && (
        <div className="flex items-center justify-between bg-pistachio-100 px-4 py-2 text-sm text-pistachio-900">
          <span>
            {updatePhase === "downloading"
              ? "Downloading update in the background…"
              : "Update installed — restart to apply it."}
          </span>
          {updatePhase === "ready" && (
            <button
              onClick={() => restartApp()}
              className="rounded-lg bg-pistachio-500 px-3 py-1 text-xs font-semibold text-white hover:bg-pistachio-600"
            >
              Restart now
            </button>
          )}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <ListSidebar
          lists={lists}
          activeListId={activeListId}
          onSelect={handleSelectList}
          onCreate={handleCreateList}
          onRename={handleRenameList}
          onDelete={handleDeleteList}
        />

        <main className="flex min-w-0 flex-1 flex-col">
          <div className="border-b border-stone-200 p-3">
            <SearchBar
              query={query}
              onChange={setQuery}
              textMode={textMode}
              onToggleTextMode={() => setTextMode((v) => !v)}
            />
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto">
            {searching && segments && (
              <div className="flex flex-wrap gap-1.5 p-4">
                {segments.map((seg, i) => (
                  <button
                    key={i}
                    onClick={() => seg.entry && setSelected(seg.entry)}
                    title={seg.entry ? seg.entry.definitions : "no dictionary entry"}
                    className={`hanzi rounded-lg border px-2.5 py-1.5 text-xl ${
                      seg.entry
                        ? "border-pistachio-300 bg-pistachio-50 text-stone-900 hover:bg-pistachio-100"
                        : "border-stone-200 bg-stone-50 text-stone-400"
                    }`}
                  >
                    {seg.surface}
                  </button>
                ))}
              </div>
            )}

            {searching && !segments && (
              <div>
                {results.length === 0 && (
                  <p className="p-6 text-sm text-stone-400">No matches.</p>
                )}
                {results.map((entry) => (
                  <button
                    key={entry.id}
                    onClick={() => setSelected(entry)}
                    className={`flex w-full items-baseline gap-4 border-b border-stone-100 px-4 py-2.5 text-left hover:bg-pistachio-50/60 ${
                      selected?.id === entry.id ? "bg-pistachio-50" : ""
                    }`}
                  >
                    <span className="hanzi w-28 shrink-0 text-xl text-stone-900">
                      {entry.simplified}
                    </span>
                    <span className="w-36 shrink-0 truncate text-sm text-pistachio-700">
                      {entry.pinyin_marks}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-sm text-stone-600">
                      {entry.definitions}
                    </span>
                  </button>
                ))}
              </div>
            )}

            {!searching && activeList && (
              <ListView
                list={activeList}
                items={listItems}
                onMove={handleMove}
                onRemove={handleRemove}
                onSelectEntry={setSelected}
                onExport={handleExport}
              />
            )}

            {!searching && !activeList && (
              <div className="flex h-full items-center justify-center p-8">
                <div className="text-center">
                  <div className="hanzi text-6xl text-pistachio-200">开心果</div>
                  <p className="mt-4 text-sm text-stone-400">
                    124,725 entries, fully offline. Search above, or open a word list.
                  </p>
                </div>
              </div>
            )}
          </div>
        </main>

        <EntryView entry={selected} lists={lists} onAdd={handleAddToList} />
      </div>

      <SettingsModal
        open={settingsOpen}
        status={syncStatus}
        onSave={handleSaveSettings}
        onClose={() => setSettingsOpen(false)}
      />

      {toast && (
        <div className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-stone-800 px-4 py-2 text-sm text-white shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}
