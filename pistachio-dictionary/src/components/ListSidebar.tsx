import type { WordList } from "../types";

interface Props {
  lists: WordList[];
  activeListId: string | null;
  onSelect: (id: string | null) => void;
  onCreate: (name: string) => void;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string) => void;
}

function SyncDot({ state }: { state: string }) {
  const color =
    state === "synced" ? "bg-pistachio-500" : state === "error" ? "bg-red-500" : "bg-amber-400";
  return <span className={`inline-block h-2 w-2 rounded-full ${color}`} title={`sync: ${state}`} />;
}

export default function ListSidebar({ lists, activeListId, onSelect, onCreate, onRename, onDelete }: Props) {
  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-stone-200 bg-stone-50">
      <div className="flex items-center justify-between px-4 pb-2 pt-4">
        <h2 className="text-sm font-semibold uppercase tracking-wide text-stone-500">
          Word lists 生词表
        </h2>
        <button
          onClick={() => {
            const name = window.prompt("New list name:");
            if (name && name.trim()) onCreate(name.trim());
          }}
          className="rounded-md bg-pistachio-500 px-2 py-1 text-xs font-semibold text-white hover:bg-pistachio-600"
        >
          + New
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-2 pb-4">
        {lists.length === 0 && (
          <p className="px-2 py-3 text-sm text-stone-400">
            No lists yet. Create one, then add words from any dictionary entry.
          </p>
        )}
        {lists.map((list) => (
          <div
            key={list.id}
            className={`group mb-1 flex cursor-pointer items-center gap-2 rounded-lg px-3 py-2 ${
              activeListId === list.id ? "bg-pistachio-100" : "hover:bg-stone-100"
            }`}
            onClick={() => onSelect(list.id)}
          >
            <SyncDot state={list.sync_state} />
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium text-stone-800">{list.name}</div>
              <div className="text-xs text-stone-400">{list.item_count} words</div>
            </div>
            <div className="hidden shrink-0 gap-1 group-hover:flex">
              <button
                title="Rename list"
                className="rounded px-1 text-stone-400 hover:bg-white hover:text-stone-700"
                onClick={(e) => {
                  e.stopPropagation();
                  const name = window.prompt("Rename list:", list.name);
                  if (name && name.trim()) onRename(list.id, name.trim());
                }}
              >
                ✎
              </button>
              <button
                title="Delete list"
                className="rounded px-1 text-stone-400 hover:bg-white hover:text-red-600"
                onClick={(e) => {
                  e.stopPropagation();
                  if (window.confirm(`Delete list “${list.name}” and its ${list.item_count} words?`))
                    onDelete(list.id);
                }}
              >
                ✕
              </button>
            </div>
          </div>
        ))}
      </div>
    </aside>
  );
}
