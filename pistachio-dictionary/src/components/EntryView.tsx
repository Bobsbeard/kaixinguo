import { useEffect, useState } from "react";
import type { EntrySummary, WordList } from "../types";

interface Props {
  entry: EntrySummary | null;
  lists: WordList[];
  onAdd: (entryId: number, listId: string) => void;
}

const LAST_LIST_KEY = "pistachio.lastListId";

export default function EntryView({ entry, lists, onAdd }: Props) {
  const [targetList, setTargetList] = useState<string>("");

  useEffect(() => {
    const remembered = window.localStorage.getItem(LAST_LIST_KEY) ?? "";
    const valid = lists.some((l) => l.id === remembered) ? remembered : lists[0]?.id ?? "";
    setTargetList(valid);
  }, [lists]);

  if (!entry) {
    return (
      <aside className="flex w-96 shrink-0 items-center justify-center border-l border-stone-200 bg-stone-50 p-6">
        <p className="text-center text-sm text-stone-400">
          Select a search result or a list item to see the full entry.
        </p>
      </aside>
    );
  }

  const senses = entry.definitions.split(" / ").filter(Boolean);

  return (
    <aside className="flex w-96 shrink-0 flex-col overflow-y-auto border-l border-stone-200 bg-stone-50 p-5">
      <div className="hanzi text-5xl leading-tight text-stone-900">{entry.simplified}</div>
      {entry.traditional !== entry.simplified && (
        <div className="hanzi mt-1 text-2xl text-stone-400">{entry.traditional}</div>
      )}
      <div className="mt-2 text-lg text-pistachio-700">{entry.pinyin_marks}</div>

      <h3 className="mt-5 text-xs font-semibold uppercase tracking-wide text-stone-400">
        Definitions
      </h3>
      <ol className="mt-2 list-decimal space-y-1.5 pl-5 text-sm leading-relaxed text-stone-700">
        {senses.map((sense, i) => (
          <li key={i}>{sense}</li>
        ))}
      </ol>

      <div className="mt-6 rounded-xl border border-pistachio-200 bg-pistachio-50 p-3">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-pistachio-700">
          Save to word list
        </h3>
        {lists.length === 0 ? (
          <p className="mt-2 text-sm text-stone-500">Create a list in the sidebar first.</p>
        ) : (
          <div className="mt-2 flex gap-2">
            <select
              value={targetList}
              onChange={(e) => setTargetList(e.target.value)}
              className="min-w-0 flex-1 rounded-lg border border-stone-300 bg-white px-2 py-1.5 text-sm"
            >
              {lists.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>
            <button
              onClick={() => {
                if (!targetList) return;
                window.localStorage.setItem(LAST_LIST_KEY, targetList);
                onAdd(entry.id, targetList);
              }}
              className="shrink-0 rounded-lg bg-pistachio-500 px-3 py-1.5 text-sm font-semibold text-white hover:bg-pistachio-600"
            >
              ＋ Add
            </button>
          </div>
        )}
      </div>

      <p className="mt-auto pt-6 text-[11px] leading-relaxed text-stone-400">
        Dictionary data: CC-CEDICT (CC BY-SA), cc-cedict.org
      </p>
    </aside>
  );
}
