import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface HistoryRetentionPolicy {
  maxItems: number;
}

const RETENTION_OPTIONS = [
  { value: 0, label: "Do not save new transcripts" },
  { value: 25, label: "Keep up to 25 transcripts" },
  { value: 50, label: "Keep up to 50 transcripts" },
  { value: 100, label: "Keep up to 100 transcripts" },
];

export function HistoryPrivacyPanel() {
  const [savedMaxItems, setSavedMaxItems] = useState(100);
  const [draftMaxItems, setDraftMaxItems] = useState(100);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    invoke<HistoryRetentionPolicy>("get_history_retention")
      .then((policy) => {
        setSavedMaxItems(policy.maxItems);
        setDraftMaxItems(policy.maxItems);
      })
      .catch((error) => setMessage(String(error)));
  }, []);

  const applyRetention = async () => {
    setIsSaving(true);
    setMessage(null);
    try {
      const policy = await invoke<HistoryRetentionPolicy>("set_history_retention", {
        maxItems: draftMaxItems,
      });
      setSavedMaxItems(policy.maxItems);
      setDraftMaxItems(policy.maxItems);
      setMessage("History retention updated.");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
        <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm font-medium text-slate-200">
          Local transcript retention
          <select
            value={draftMaxItems}
            onChange={(event) => setDraftMaxItems(Number(event.target.value))}
            className="focus-ring min-h-9 w-full rounded-lg border border-white/10 bg-[#111] px-2.5 text-sm text-white"
          >
            {RETENTION_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>{option.label}</option>
            ))}
          </select>
        </label>
        <button
          type="button"
          onClick={() => void applyRetention()}
          disabled={isSaving || draftMaxItems === savedMaxItems}
          className="focus-ring min-h-9 rounded-full border border-primary/30 bg-primary/10 px-3 text-xs font-semibold text-primary transition-colors hover:bg-primary/20 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isSaving ? "Applying…" : "Apply retention"}
        </button>
      </div>
      <p className="text-[11px] leading-5 text-slate-400">
        History is encrypted locally. Lowering this limit affects future additions and does not silently delete existing entries. Use Clear history in the main window for a permanent purge of history and recovery copies; exported files remain outside FamVoice.
      </p>
      {message ? <p className="break-words text-xs text-slate-300" role="status">{message}</p> : null}
    </div>
  );
}
