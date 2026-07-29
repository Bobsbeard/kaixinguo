import type { SyncSettingsView } from "../types";

interface Props {
  status: SyncSettingsView | null;
  syncing: boolean;
  onSyncNow: () => void;
  onOpenSettings: () => void;
}

export default function SyncBar({ status, syncing, onSyncNow, onOpenSettings }: Props) {
  const pending = status?.pending_ops ?? 0;
  const hasError = Boolean(status?.last_error);
  const configured = Boolean(status?.server_url);

  const dot = !configured
    ? "bg-stone-300"
    : hasError
      ? "bg-red-500"
      : pending > 0
        ? "bg-amber-400"
        : "bg-pistachio-500";

  const label = !configured
    ? "Sync not configured"
    : hasError
      ? `Sync error: ${status?.last_error}`
      : pending > 0
        ? `${pending} change(s) pending`
        : status?.last_sync_at
          ? `Synced ${new Date(status.last_sync_at).toLocaleTimeString()}`
          : "Ready to sync";

  return (
    <div className="flex items-center gap-3">
      <div className="flex items-center gap-2" title={label}>
        <span className={`h-2.5 w-2.5 rounded-full ${dot}`} />
        <span className="max-w-72 truncate text-xs text-stone-500">{label}</span>
      </div>
      <button
        onClick={onSyncNow}
        disabled={syncing || !configured}
        className="rounded-lg border border-stone-300 bg-white px-3 py-1.5 text-xs font-medium text-stone-600 shadow-sm hover:bg-pistachio-50 disabled:opacity-40"
      >
        {syncing ? "Syncing…" : "Sync now"}
      </button>
      <button
        onClick={onOpenSettings}
        title="Bingqilin sync settings"
        className="rounded-lg border border-stone-300 bg-white px-2.5 py-1.5 text-xs text-stone-600 shadow-sm hover:bg-pistachio-50"
      >
        ⚙
      </button>
    </div>
  );
}
