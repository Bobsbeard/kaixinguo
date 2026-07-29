import type { EntrySummary, ListItemView, WordList } from "../types";

interface Props {
  list: WordList;
  items: ListItemView[];
  onMove: (itemId: string, newIndex: number) => void;
  onRemove: (itemId: string) => void;
  onSelectEntry: (entry: EntrySummary) => void;
  onExport: () => void;
}

function SyncDot({ state }: { state: string }) {
  const color =
    state === "synced" ? "bg-pistachio-500" : state === "error" ? "bg-red-500" : "bg-amber-400";
  return <span className={`inline-block h-2 w-2 rounded-full ${color}`} title={`sync: ${state}`} />;
}

export default function ListView({ list, items, onMove, onRemove, onSelectEntry, onExport }: Props) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-stone-200 px-4 py-3">
        <div>
          <h2 className="text-lg font-semibold text-stone-800">{list.name}</h2>
          <p className="text-xs text-stone-400">
            {items.length} words · ordered list ·{" "}
            {list.sync_state === "synced" ? "synced to Bingqilin" : `sync ${list.sync_state}`}
          </p>
        </div>
        <button
          onClick={onExport}
          disabled={items.length === 0}
          className="rounded-lg border border-stone-300 bg-white px-3 py-1.5 text-sm text-stone-600 shadow-sm hover:bg-pistachio-50 disabled:opacity-40"
        >
          Export TSV
        </button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {items.length === 0 && (
          <p className="p-6 text-sm text-stone-400">
            Empty list. Search for a word, open it, and press “＋ Add to list”.
          </p>
        )}
        {items.map((item, idx) => (
          <div
            key={item.item_id}
            className="flex items-center gap-3 border-b border-stone-100 px-4 py-2 hover:bg-pistachio-50/50"
          >
            <span className="w-7 text-right text-xs tabular-nums text-stone-400">{idx + 1}</span>
            <SyncDot state={item.sync_state} />
            <button
              className="hanzi w-24 shrink-0 text-left text-lg text-stone-900 hover:text-pistachio-700"
              onClick={() => onSelectEntry(item.entry)}
              title="Open in dictionary"
            >
              {item.entry.simplified}
            </button>
            <span className="w-32 shrink-0 truncate text-sm text-stone-500">
              {item.entry.pinyin_marks}
            </span>
            <span className="min-w-0 flex-1 truncate text-sm text-stone-600" title={item.entry.definitions}>
              {item.entry.definitions}
            </span>
            <div className="flex shrink-0 items-center gap-0.5">
              <button
                title="Move up"
                disabled={idx === 0}
                onClick={() => onMove(item.item_id, idx - 1)}
                className="rounded px-1.5 py-0.5 text-stone-400 hover:bg-white hover:text-stone-800 disabled:opacity-25"
              >
                ↑
              </button>
              <button
                title="Move down"
                disabled={idx === items.length - 1}
                onClick={() => onMove(item.item_id, idx + 1)}
                className="rounded px-1.5 py-0.5 text-stone-400 hover:bg-white hover:text-stone-800 disabled:opacity-25"
              >
                ↓
              </button>
              <button
                title="Remove from list"
                onClick={() => onRemove(item.item_id)}
                className="rounded px-1.5 py-0.5 text-stone-400 hover:bg-white hover:text-red-600"
              >
                ✕
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
