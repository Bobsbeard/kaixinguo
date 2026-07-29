interface Props {
  query: string;
  onChange: (value: string) => void;
  textMode: boolean;
  onToggleTextMode: () => void;
}

export default function SearchBar({ query, onChange, textMode, onToggleTextMode }: Props) {
  return (
    <div className="flex items-center gap-2">
      <input
        autoFocus
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder={textMode ? "Paste Chinese text to segment…" : "Search 中文 / pinyin / English…"}
        className="w-full rounded-lg border border-stone-300 bg-white px-4 py-2.5 text-lg shadow-sm outline-none focus:border-pistachio-500 focus:ring-2 focus:ring-pistachio-200"
      />
      <button
        onClick={onToggleTextMode}
        title="Text mode: segment pasted Chinese text into words"
        className={`shrink-0 rounded-lg border px-3 py-2.5 text-sm font-medium shadow-sm transition-colors ${
          textMode
            ? "border-pistachio-600 bg-pistachio-500 text-white"
            : "border-stone-300 bg-white text-stone-600 hover:bg-pistachio-50"
        }`}
      >
        文 Text
      </button>
    </div>
  );
}
