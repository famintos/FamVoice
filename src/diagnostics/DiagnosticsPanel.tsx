import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Download, Mic2, Network } from "lucide-react";
import type {
  DiagnosticsSnapshot,
  MicrophoneSignalTest,
  ProviderConnectivityTest,
} from "./types";

const controlMotion = "transition-colors duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)]";

function statusTone(status: "ok" | "warning" | "error"): string {
  return status === "ok"
    ? "bg-green-400"
    : status === "warning"
      ? "bg-primary"
      : "bg-danger";
}

function DiagnosticRow({
  label,
  value,
  status,
}: {
  label: string;
  value: string;
  status: "ok" | "warning" | "error";
}) {
  return (
    <div className="flex min-h-8 items-center justify-between gap-4 border-b border-white/[0.06] py-1.5 last:border-b-0">
      <span className="text-xs text-slate-400">{label}</span>
      <span className="flex min-w-0 items-center gap-2 text-right text-xs font-medium text-slate-200">
        <span className={`size-1.5 shrink-0 rounded-full ${statusTone(status)}`} aria-hidden="true" />
        <span className="truncate">{value}</span>
      </span>
    </div>
  );
}

export function DiagnosticsPanel() {
  const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot | null>(null);
  const [isTestingMicrophone, setIsTestingMicrophone] = useState(false);
  const [isTestingProvider, setIsTestingProvider] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setSnapshot(await invoke<DiagnosticsSnapshot>("get_diagnostics_snapshot"));
    } catch (error) {
      setMessage(String(error));
    }
  };

  useEffect(() => {
    void invoke<DiagnosticsSnapshot>("get_diagnostics_snapshot")
      .then(setSnapshot)
      .catch((error) => setMessage(String(error)));
    const interval = window.setInterval(() => void refresh(), 3_000);
    return () => window.clearInterval(interval);
  }, []);

  const testMicrophone = async () => {
    setIsTestingMicrophone(true);
    setMessage(null);
    try {
      const result = await invoke<MicrophoneSignalTest>("run_microphone_test");
      setMessage(result.signalDetected
        ? "Microphone signal detected."
        : "The microphone opened, but no clear signal was detected.");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setIsTestingMicrophone(false);
    }
  };

  const testProvider = async () => {
    setIsTestingProvider(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderConnectivityTest>("test_provider_auth");
      setMessage(result.authenticated
        ? `${result.provider} authenticated in ${result.latencyMs} ms.`
        : result.error ?? "Provider authentication failed.");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setIsTestingProvider(false);
    }
  };

  const exportReport = async () => {
    setMessage(null);
    try {
      const path = await invoke<string>("export_diagnostics");
      setMessage(`Sanitized report saved to ${path}`);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const signalPercent = Math.round(Math.min(1, Math.max(0, snapshot?.microphoneTest?.peak ?? 0)) * 100);
  const buttonClassName = `focus-ring inline-flex min-h-8 items-center justify-center gap-1.5 rounded-full border border-white/10 bg-black/20 px-3 text-xs font-medium text-slate-200 ${controlMotion} hover:border-primary/40 hover:text-white disabled:cursor-not-allowed disabled:opacity-50`;

  if (!snapshot) {
    return <p className="text-xs text-slate-400">Loading technical status…</p>;
  }

  return (
    <div className="space-y-3">
      <div className="rounded-xl border border-white/[0.08] bg-black/15 px-3">
        <DiagnosticRow
          label="Microphone"
          value={snapshot.device.connected
            ? snapshot.device.selectedLabel ?? "System default"
            : "Selected device disconnected"}
          status={snapshot.device.status}
        />
        <DiagnosticRow
          label="Recording hotkey"
          value={snapshot.hotkey.conflict
            ? "Conflicts with re-paste"
            : snapshot.hotkey.recordingAvailable
              ? snapshot.hotkey.recordingHotkey
              : "Unavailable"}
          status={snapshot.hotkey.status}
        />
        <DiagnosticRow
          label="Transcription provider"
          value={`${snapshot.provider.provider} · ${snapshot.provider.model}`}
          status={snapshot.provider.status}
        />
        <DiagnosticRow
          label="Runtime"
          value={`v${snapshot.version.appVersion} · ${snapshot.version.architecture}`}
          status="ok"
        />
      </div>

      {snapshot.microphoneTest ? (
        <div className="space-y-1" aria-label="Last microphone signal level">
          <div className="flex items-center justify-between text-[11px] text-slate-400">
            <span>Last signal test</span>
            <span className="font-mono">{signalPercent}% peak</span>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-white/[0.08]">
            <div
              className="h-full rounded-full bg-primary transition-[width] duration-200"
              style={{ width: `${signalPercent}%` }}
            />
          </div>
        </div>
      ) : null}

      {snapshot.lastOperation ? (
        <p className="flex items-center gap-1.5 text-[11px] leading-5 text-slate-400">
          <Activity size={12} aria-hidden="true" />
          Last {snapshot.lastOperation.operation.replace(/_/g, " ")}:
          {` ${snapshot.lastOperation.latencyMs} ms`}
          {snapshot.lastOperation.error ? ` · ${snapshot.lastOperation.error}` : ""}
        </p>
      ) : null}

      <div className="flex flex-wrap gap-2">
        <button type="button" className={buttonClassName} onClick={() => void testMicrophone()} disabled={isTestingMicrophone}>
          <Mic2 size={12} aria-hidden="true" />
          {isTestingMicrophone ? "Listening…" : "Test microphone"}
        </button>
        <button type="button" className={buttonClassName} onClick={() => void testProvider()} disabled={isTestingProvider}>
          <Network size={12} aria-hidden="true" />
          {isTestingProvider ? "Testing…" : "Test provider"}
        </button>
        <button type="button" className={buttonClassName} onClick={() => void exportReport()}>
          <Download size={12} aria-hidden="true" />
          Export report
        </button>
      </div>

      <p className="text-[11px] leading-5 text-slate-400">
        Provider test uses the saved credentials and sends no recording or transcript. Exports omit credentials, transcripts, device identifiers, device names, and literal hotkeys.
      </p>
      {message ? <p className="break-words text-xs leading-5 text-slate-300" role="status">{message}</p> : null}
    </div>
  );
}
