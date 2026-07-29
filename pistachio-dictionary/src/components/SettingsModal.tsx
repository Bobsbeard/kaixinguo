import { useEffect, useState } from "react";
import type { SyncSettingsView } from "../types";

interface Props {
  open: boolean;
  status: SyncSettingsView | null;
  onSave: (serverUrl: string, token: string) => void;
  onClose: () => void;
}

export default function SettingsModal({ open, status, onSave, onClose }: Props) {
  const [url, setUrl] = useState("");
  const [token, setToken] = useState("");

  useEffect(() => {
    if (open) {
      setUrl(status?.server_url ?? "");
      setToken("");
    }
  }, [open, status]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30"
      onClick={onClose}
    >
      <div
        className="w-[28rem] rounded-2xl bg-white p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-stone-800">Bingqilin sync</h2>
        <p className="mt-1 text-sm text-stone-500">
          Connect to your Bingqilin server to sync word lists when online. Everything works
          offline regardless.
        </p>

        <label className="mt-4 block text-sm font-medium text-stone-600">Server URL</label>
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://your-bingqilin-server.example"
          className="mt-1 w-full rounded-lg border border-stone-300 px-3 py-2 text-sm outline-none focus:border-pistachio-500"
        />

        <label className="mt-3 block text-sm font-medium text-stone-600">
          Auth token {status?.has_token && <span className="text-stone-400">(saved — leave blank to keep)</span>}
        </label>
        <input
          value={token}
          onChange={(e) => setToken(e.target.value)}
          type="password"
          placeholder="••••••••"
          className="mt-1 w-full rounded-lg border border-stone-300 px-3 py-2 text-sm outline-none focus:border-pistachio-500"
        />

        <p className="mt-3 rounded-lg bg-pistachio-50 px-3 py-2 text-xs text-pistachio-800">
          Tip: enter <code className="font-mono">mock</code> as the URL to test the sync pipeline
          without a real server.
        </p>

        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-lg border border-stone-300 px-4 py-2 text-sm text-stone-600 hover:bg-stone-50"
          >
            Cancel
          </button>
          <button
            onClick={() => onSave(url.trim(), token)}
            className="rounded-lg bg-pistachio-500 px-4 py-2 text-sm font-semibold text-white hover:bg-pistachio-600"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
