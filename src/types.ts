export interface EntrySummary {
  id: number;
  traditional: string;
  simplified: string;
  pinyin_marks: string;
  definitions: string;
}

export interface Segment {
  surface: string;
  entry: EntrySummary | null;
}

export interface WordList {
  id: string;
  name: string;
  item_count: number;
  updated_at: string;
  sync_state: string;
}

export interface ListItemView {
  item_id: string;
  position: number;
  sync_state: string;
  entry: EntrySummary;
}

export interface SyncSettingsView {
  server_url: string;
  has_token: boolean;
  pending_ops: number;
  last_sync_at: string | null;
  last_error: string | null;
}

export interface SyncReport {
  pushed: number;
  failed: number;
  pending: number;
  message: string;
  synced_at: string | null;
}
