import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  Plus,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
  X,
} from "lucide-react";
import {
  DEFAULT_HOTKEY,
  LANGUAGES,
  MODELS_BY_PROVIDER,
  PROMPT_OPTIMIZER_MODELS,
  TRANSCRIPTION_PROVIDERS,
} from "./appConstants";
import { buildHotkeyString, formatHotkey } from "./appHelpers";
import type { Replacement, SaveSettingsPayload, SettingsViewModel } from "./appTypes";

interface ReplacementDraft extends Replacement {
  id: string;
}

type SettingsDraft = Omit<SettingsViewModel, "replacements"> & {
  replacements: ReplacementDraft[];
};

let replacementDraftCounter = 0;

function nextReplacementDraftId(): string {
  replacementDraftCounter += 1;
  return `replacement-${replacementDraftCounter}`;
}

function toReplacementDraft(replacement: Replacement): ReplacementDraft {
  return {
    ...replacement,
    id: nextReplacementDraftId(),
  };
}

function normalizePromptOptimizerModel(model: string): string {
  return PROMPT_OPTIMIZER_MODELS.some((option) => option.value === model)
    ? model
    : PROMPT_OPTIMIZER_MODELS[0].value;
}

function toSettingsDraft(settings: SettingsViewModel): SettingsDraft {
  return {
    ...settings,
    prompt_optimizer_model: normalizePromptOptimizerModel(settings.prompt_optimizer_model),
    replacements: settings.replacements.map(toReplacementDraft),
  };
}

function toSavePayload(
  settings: SettingsDraft,
  apiKeyInput: string,
  groqApiKeyInput: string,
): SaveSettingsPayload {
  return {
    ...settings,
    api_key: apiKeyInput.trim() ? apiKeyInput.trim() : null,
    groq_api_key: groqApiKeyInput.trim() ? groqApiKeyInput.trim() : null,
    replacements: settings.replacements.map(({ id: _id, ...replacement }) => replacement),
  };
}

function SettingsShell({
  children,
  onClose,
}: {
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <main
      data-tauri-drag-region
      className="signal-shell signal-shell--settings relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[28px]"
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(209,122,40,0.1),transparent_36%),linear-gradient(180deg,rgba(255,255,255,0.02),transparent_16%)]"
      />
      <div className="relative z-10 flex items-center justify-between border-b border-white/6 px-4 py-2.5">
        <div className="flex items-center gap-2 pointer-events-none select-none text-slate-300">
          <SettingsIcon size={14} className="text-primary" />
          <div className="flex flex-col">
            <span className="font-mono text-[10px] uppercase tracking-[0.24em] text-slate-500">
              Signal Console
            </span>
            <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-200">
              Settings
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="rounded-full p-1.5 text-slate-500 transition-colors hover:bg-white/10 hover:text-white"
          aria-label="Close settings"
        >
          <X size={14} />
        </button>
      </div>
      <div className="relative z-10 flex-1 min-h-0 overflow-hidden">
        {children}
      </div>
    </main>
  );
}

function ControlSection({
  eyebrow,
  description,
  action,
  children,
}: {
  eyebrow: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="control-section rounded-[20px] border border-white/8 bg-black/18 px-3 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <p className="section-eyebrow font-mono text-[10px] uppercase tracking-[0.24em] text-slate-500">
            {eyebrow}
          </p>
          {description ? (
            <p className="max-w-[42rem] text-[10px] leading-4 text-slate-500">
              {description}
            </p>
          ) : null}
        </div>
        {action ? <div className="flex-shrink-0">{action}</div> : null}
      </div>
      <div className="mt-3 space-y-3">{children}</div>
    </section>
  );
}

export function SettingsView() {
  const [settings, setSettings] = useState<SettingsDraft | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [groqApiKeyInput, setGroqApiKeyInput] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [autostartAvailable, setAutostartAvailable] = useState(true);
  const [isListening, setIsListening] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("");
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [isCheckingForUpdates, setIsCheckingForUpdates] = useState(false);
  const [isApplyingUpdate, setIsApplyingUpdate] = useState(false);
  const [updateCheckError, setUpdateCheckError] = useState<string | null>(null);
  const [updateInstallError, setUpdateInstallError] = useState<string | null>(null);
  const appWindow = getCurrentWindow();
  const updateCheckRequestIdRef = useRef(0);

  const refreshUpdate = async () => {
    const requestId = ++updateCheckRequestIdRef.current;
    setIsCheckingForUpdates(true);
    setUpdateCheckError(null);
    try {
      const update = await check();
      if (requestId !== updateCheckRequestIdRef.current) {
        return;
      }
      setAvailableUpdate(update);
      setUpdateCheckError(null);
    } catch (error) {
      if (requestId !== updateCheckRequestIdRef.current) {
        return;
      }
      console.error("Update check failed:", error);
      setAvailableUpdate(null);
      setUpdateCheckError(String(error));
    } finally {
      if (requestId === updateCheckRequestIdRef.current) {
        setIsCheckingForUpdates(false);
      }
    }
  };

  useEffect(() => {
    invoke<SettingsViewModel>("get_settings")
      .then((loadedSettings) => {
        setSettings(toSettingsDraft(loadedSettings));
        setApiKeyInput("");
        setGroqApiKeyInput("");
      })
      .catch((error) => {
        console.error("Failed to load settings:", error);
        setErrorMessage(String(error));
      });

    isEnabled()
      .then(setAutostart)
      .catch((error) => {
        console.error("Autostart status error:", error);
      });

    invoke<boolean>("can_manage_autostart")
      .then((available) => {
        setAutostartAvailable(available);
        if (!available) {
          setAutostart(false);
        }
      })
      .catch((error) => {
        console.error("Autostart availability error:", error);
      });

    getVersion()
      .then(setAppVersion)
      .catch((error) => {
        console.error("Failed to load app version:", error);
      });

    void refreshUpdate();

    const unlistenFocusChanged = appWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void refreshUpdate();
      }
    });

    return () => {
      unlistenFocusChanged.then((fn) => fn());
    };
  }, []);

  const saveSettings = async (newSettings: SettingsDraft) => {
    try {
      setErrorMessage(null);
      const payload = toSavePayload(
        newSettings,
        apiKeyInput,
        groqApiKeyInput,
      );
      const savedSettings = await invoke<SettingsViewModel>("save_settings", { newSettings: payload });
      setSettings(toSettingsDraft(savedSettings));
      setApiKeyInput("");
      setGroqApiKeyInput("");
      try {
        if (autostartAvailable && autostart) {
          await enable();
        } else {
          await disable();
        }
      } catch (error) {
        console.error("Autostart error:", error);
      }
      await invoke("close_settings_window");
    } catch (error) {
      console.error("Failed to save settings:", error);
      setErrorMessage(String(error));
    }
  };

  const handleApplyUpdate = async () => {
    if (!availableUpdate || isApplyingUpdate) return;

    try {
      setUpdateInstallError(null);
      setIsApplyingUpdate(true);
      await availableUpdate.downloadAndInstall();
      await relaunch();
    } catch (error) {
      console.error("Failed to apply update:", error);
      setUpdateInstallError(String(error));
      setIsApplyingUpdate(false);
    }
  };

  const addReplacement = () => {
    if (!settings) return;
    setSettings({
      ...settings,
      replacements: [
        ...settings.replacements,
        { id: nextReplacementDraftId(), target: "", replacement: "" },
      ],
    });
  };

  const removeReplacement = (id: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      replacements: settings.replacements.filter((replacement) => replacement.id !== id),
    });
  };

  const updateReplacement = (
    id: string,
    field: keyof Replacement,
    value: string,
  ) => {
    if (!settings) return;
    setSettings({
      ...settings,
      replacements: settings.replacements.map((replacement) => (
        replacement.id === id
          ? { ...replacement, [field]: value }
          : replacement
      )),
    });
  };

  const handleMouseCapture = (e: MouseEvent<HTMLInputElement>) => {
    if (!isListening || !settings) return;

    if (e.button === 1 || e.button === 3 || e.button === 4) {
      e.preventDefault();
      const combo = e.button === 1 ? "Mouse3" : e.button === 3 ? "Mouse4" : "Mouse5";
      setSettings({ ...settings, hotkey: combo });
      setIsListening(false);
      (e.target as HTMLInputElement).blur();
    }
  };

  const handleHotkeyCapture = (e: KeyboardEvent<HTMLInputElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const combo = buildHotkeyString(e);
    if (combo && settings) {
      setSettings({ ...settings, hotkey: combo });
      setIsListening(false);
      (e.target as HTMLInputElement).blur();
    }
  };

  const closeSettingsWindow = () => invoke("close_settings_window");
  const currentVersionRow = (
    <div className="flex items-center justify-between text-xs text-slate-300">
      <span>Current version</span>
      <span className="text-slate-400">{appVersion ? `v${appVersion}` : "Loading..."}</span>
    </div>
  );

  if (!settings) {
    return (
      <SettingsShell onClose={closeSettingsWindow}>
        <div className="flex h-full items-center justify-center px-4 py-4 text-center text-xs no-drag">
          {errorMessage ? (
            <div
              className="status-panel status-panel--error rounded-[18px] border border-red-500/25 bg-red-500/10 px-3 py-3 text-red-100"
              style={{ borderRadius: 18, overflow: "hidden" }}
            >
              <p className="section-eyebrow font-mono text-[10px] uppercase tracking-[0.24em] text-red-200/80">
                Settings load error
              </p>
              <p className="mt-2 text-sm leading-5 text-red-50">{errorMessage}</p>
            </div>
          ) : (
            <div
              className="status-panel status-panel--neutral rounded-[18px] border border-white/10 bg-black/25 px-3 py-3 text-slate-200"
              style={{ borderRadius: 18, overflow: "hidden" }}
            >
              <p className="section-eyebrow font-mono text-[10px] uppercase tracking-[0.24em] text-slate-500">
                Loading
              </p>
              <p className="mt-2 text-sm leading-5 text-slate-100">Loading settings...</p>
            </div>
          )}
        </div>
      </SettingsShell>
    );
  }

  return (
    <SettingsShell onClose={closeSettingsWindow}>
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden px-3 py-3 pr-2 custom-scrollbar no-drag">
          <div className="space-y-4 pb-4">
            <ControlSection eyebrow="Transcription">
          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Provider
            <select
              value={settings.transcription_provider}
              onChange={(e) => {
                const provider = e.target.value;
                const models = MODELS_BY_PROVIDER[provider] ?? [];
                setSettings({
                  ...settings,
                  transcription_provider: provider,
                  model: models[0]?.value ?? "",
                });
              }}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              {TRANSCRIPTION_PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            <div className="flex items-center justify-between">
              <span>OpenAI API Key</span>
              <span className={`flex items-center gap-1 text-[10px] font-medium ${settings.api_key_present ? "text-green-400" : "text-slate-500"}`}>
                <span className={`w-1.5 h-1.5 rounded-full ${settings.api_key_present ? "bg-green-400" : "bg-slate-500"}`} />
                {settings.api_key_present ? "Configured" : "Not set"}
              </span>
            </div>
            <input
              type="password"
              value={apiKeyInput}
              onChange={(e) => setApiKeyInput(e.target.value)}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full"
              placeholder={settings.api_key_masked ?? "sk-..."}
            />
            <span className="text-[10px] text-gray-500">
              {settings.api_key_present
                ? `Saved in your OS credential store as ${settings.api_key_masked}. Leave blank to keep it.`
                : "Used for OpenAI transcription and prompt optimization. Saved after you enter one."}
            </span>
          </label>

          {settings.transcription_provider === "groq" && (
            <label className="text-xs text-gray-400 flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <span>Groq API Key</span>
                <span className={`flex items-center gap-1 text-[10px] font-medium ${settings.groq_api_key_present ? "text-green-400" : "text-slate-500"}`}>
                  <span className={`w-1.5 h-1.5 rounded-full ${settings.groq_api_key_present ? "bg-green-400" : "bg-slate-500"}`} />
                  {settings.groq_api_key_present ? "Configured" : "Not set"}
                </span>
              </div>
              <input
                type="password"
                value={groqApiKeyInput}
                onChange={(e) => setGroqApiKeyInput(e.target.value)}
                className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full"
                placeholder={settings.groq_api_key_masked ?? "gsk_..."}
              />
              <span className="text-[10px] text-gray-500">
                {settings.groq_api_key_present
                  ? `Saved in your OS credential store as ${settings.groq_api_key_masked}. Leave blank to keep it.`
                  : "Saved in your OS credential store after you enter one."}
              </span>
            </label>
          )}

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Model
            <select
              value={settings.model}
              onChange={(e) => setSettings({ ...settings, model: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              {(MODELS_BY_PROVIDER[settings.transcription_provider] ?? []).map((model) => (
                <option key={model.value} value={model.value}>{model.label}</option>
              ))}
            </select>
          </label>
          </ControlSection>

          <ControlSection
            eyebrow="Prompt Optimization"
            description="Runs a second OpenAI pass after transcription to rewrite the finalized transcript into an English implementation prompt for coding agents."
          >

          <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
            <input
              type="checkbox"
              checked={settings.prompt_optimization_enabled}
              onChange={(e) => setSettings({ ...settings, prompt_optimization_enabled: e.target.checked })}
              className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
            />
            <div className="flex flex-col">
              <span>Improve into prompt</span>
              <span className="text-[10px] text-gray-500">Adds an extra OpenAI model pass that rewrites the finalized transcript into an English implementation prompt for a coding agent.</span>
            </div>
          </label>

          <label className="text-xs text-gray-400 flex flex-col gap-1.5">
            Model
            <select
              value={settings.prompt_optimizer_model}
              onChange={(e) => setSettings({ ...settings, prompt_optimizer_model: e.target.value })}
              className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
            >
              {PROMPT_OPTIMIZER_MODELS.map((model) => (
                <option key={model.value} value={model.value}>{model.label}</option>
              ))}
            </select>
          </label>

          <p className="text-[10px] text-gray-500">
            Uses the saved OpenAI API key above. Keep the static metaprompt first and the dictated request last to maximize prompt caching.
          </p>
          </ControlSection>

          <ControlSection eyebrow="Behavior">
          <div className="flex flex-col gap-3">
            <div className="text-xs text-gray-400 flex flex-col gap-1.5">
              Hotkey
              <div className="flex gap-2 items-center">
                <input
                  type="text"
                  readOnly
                  value={isListening ? "Press keys..." : formatHotkey(settings.hotkey)}
                  onFocus={() => setIsListening(true)}
                  onBlur={() => setIsListening(false)}
                  onKeyDown={handleHotkeyCapture}
                  onMouseDown={handleMouseCapture}
                  onContextMenu={(e) => isListening && e.preventDefault()}
                  className={`flex-1 p-2 bg-black/40 rounded border text-xs text-white focus:outline-none transition-colors w-full cursor-pointer ${isListening ? "border-primary bg-primary/10" : "border-white/10"
                    }`}
                />
                <button
                  type="button"
                  onClick={() => setSettings({ ...settings, hotkey: DEFAULT_HOTKEY })}
                  title="Reset to default"
                  className="p-2 bg-black/40 rounded border border-white/10 text-gray-400 hover:text-primary hover:border-primary/50 transition-colors cursor-pointer"
                >
                  <RefreshCw size={12} />
                </button>
              </div>
            </div>
            <label className="text-xs text-gray-400 flex flex-col gap-1.5">
              Language Preference
              <select
                value={settings.language}
                onChange={(e) => setSettings({ ...settings, language: e.target.value })}
                className="p-2 bg-black/40 rounded border border-white/10 text-xs text-white focus:outline-none focus:border-primary transition-colors w-full cursor-pointer"
              >
                {LANGUAGES.map((lang) => (
                  <option key={lang.value} value={lang.value}>{lang.label}</option>
                ))}
              </select>
              <span className="text-[10px] text-gray-500">
                Auto Detect handles mixed dictation. Choose Portuguese or English only if you want to bias transcription toward one language.
              </span>
            </label>

            <label className="text-xs text-gray-400 flex flex-col gap-1.5">
              Mic Sensitivity
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={settings.mic_sensitivity}
                  onChange={(e) => setSettings({ ...settings, mic_sensitivity: Number(e.target.value) })}
                  className="flex-1 accent-primary cursor-pointer"
                />
                <span className="w-8 text-right text-[10px] text-gray-500">
                  {settings.mic_sensitivity}
                </span>
              </div>
              <div className="flex justify-between text-[10px] text-gray-600">
                <span>Less noise</span>
                <span>Quieter voice</span>
              </div>
              <span className="text-[10px] text-gray-500">
                Higher sensitivity helps softer speech, but can pick up more background noise.
              </span>
            </label>
          </div>

          <div className="space-y-2">
            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.widget_mode}
                onChange={(e) => setSettings({ ...settings, widget_mode: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Widget Mode</span>
                <span className="text-[10px] text-gray-500">Minimal UI with only waveforms</span>
              </div>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.auto_paste}
                onChange={(e) => setSettings({ ...settings, auto_paste: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <span>Auto Paste Transcript</span>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={settings.preserve_clipboard}
                onChange={(e) => setSettings({ ...settings, preserve_clipboard: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Preserve Clipboard</span>
                <span className="text-[10px] text-gray-500">Restore the original clipboard after a successful auto-paste</span>
              </div>
            </label>

            <label className="flex items-center gap-3 text-xs text-gray-300 cursor-pointer hover:text-white transition-colors">
              <input
                type="checkbox"
                checked={autostart}
                onChange={(e) => setAutostart(e.target.checked)}
                disabled={!autostartAvailable}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <span>Launch on Startup</span>
            </label>
            {!autostartAvailable && (
              <p className="text-[10px] text-gray-500 pl-7">
                Launch on Startup is only available from the installed app.
              </p>
            )}
          </div>
          </ControlSection>

          <ControlSection
            eyebrow="Update"
            action={
              <button
                onClick={() => void refreshUpdate()}
                className="flex items-center gap-1 text-[10px] text-slate-400 transition-colors cursor-pointer hover:text-primary"
                type="button"
              >
                <RefreshCw size={10} />
                Refresh
              </button>
            }
          >
            <div className="space-y-3">
              {isCheckingForUpdates ? (
                <div
                  className="status-panel status-panel--neutral rounded-[18px] border border-white/10 bg-black/25 px-3 py-3 text-slate-200"
                  style={{ borderRadius: 18, overflow: "hidden" }}
                >
                  {currentVersionRow}
                  <p className="mt-3 text-[11px] leading-5 text-slate-400">
                    Checking for updates...
                  </p>
                </div>
              ) : updateCheckError ? (
                <div
                  className="status-panel status-panel--error rounded-[18px] border border-red-500/25 bg-red-500/10 px-3 py-3 text-red-100"
                  style={{ borderRadius: 18, overflow: "hidden" }}
                >
                  {currentVersionRow}
                  <p className="mt-3 text-[11px] font-medium text-red-100">
                    Unable to check for updates right now.
                  </p>
                  <p className="mt-1 text-[11px] leading-5 text-red-100/90">
                    {updateCheckError}
                  </p>
                </div>
              ) : availableUpdate ? (
                <div
                  className="status-panel status-panel--neutral rounded-[18px] border border-white/10 bg-black/25 px-3 py-3 text-slate-200"
                  style={{ borderRadius: 18, overflow: "hidden" }}
                >
                  {currentVersionRow}
                  <div className="mt-3 flex items-center justify-between text-xs text-slate-300">
                    <span>Update available</span>
                    <span className="font-mono text-slate-400">v{availableUpdate.version}</span>
                  </div>
                  <button
                    type="button"
                    onClick={handleApplyUpdate}
                    disabled={isApplyingUpdate}
                    className="mt-3 w-full rounded-lg border border-primary/30 bg-primary/10 py-2 text-xs font-semibold text-amber-50 transition-colors hover:bg-primary/20 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {isApplyingUpdate ? "Updating..." : "Update"}
                  </button>
                </div>
              ) : (
                <div
                  className="status-panel status-panel--neutral rounded-[18px] border border-white/10 bg-black/25 px-3 py-3 text-slate-200"
                  style={{ borderRadius: 18, overflow: "hidden" }}
                >
                  {currentVersionRow}
                  <p className="mt-3 text-[11px] leading-5 text-slate-400">
                    No update available.
                  </p>
                </div>
              )}

              {updateInstallError && (
                <div
                  className="status-panel status-panel--error rounded-[18px] border border-red-500/25 bg-red-500/10 px-3 py-3 text-red-100"
                  style={{ borderRadius: 18, overflow: "hidden" }}
                >
                  <p className="text-[11px] font-medium text-red-100">
                    Update installation failed.
                  </p>
                  <p className="mt-1 text-[11px] leading-5 text-red-100/90">
                    {updateInstallError}
                  </p>
                </div>
              )}
            </div>
          </ControlSection>

          <ControlSection
            eyebrow="Glossary"
            action={
              <button
                onClick={addReplacement}
                className="flex items-center gap-1 text-[10px] text-primary transition-colors cursor-pointer hover:text-amber-200"
              >
                <Plus size={10} /> Add
              </button>
            }
          >
            <div className="space-y-2">
              {settings.replacements.map((replacement) => (
                <div key={replacement.id} className="flex items-center gap-2">
                  <input
                    value={replacement.target}
                    onChange={(e) => updateReplacement(replacement.id, "target", e.target.value)}
                    placeholder="spoken term"
                    className="flex-1 min-w-0 rounded border border-white/10 bg-black/40 p-1.5 text-[10px] text-white transition-colors focus:border-primary focus:outline-none"
                  />
                  <span className="text-[10px] text-gray-500">-&gt;</span>
                  <input
                    value={replacement.replacement}
                    onChange={(e) => updateReplacement(replacement.id, "replacement", e.target.value)}
                    placeholder="preferred text"
                    className="flex-1 min-w-0 rounded border border-white/10 bg-black/40 p-1.5 text-[10px] text-white transition-colors focus:border-primary focus:outline-none"
                  />
                  <button
                    onClick={() => removeReplacement(replacement.id)}
                    className="cursor-pointer p-1 text-gray-600 transition-colors hover:text-red-400"
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
              {settings.replacements.length === 0 && (
                <p className="text-[10px] text-gray-600 italic">No glossary entries configured.</p>
              )}
            </div>
          </ControlSection>
        </div>
      </div>

      <div className="border-t border-white/6 px-4 py-3 no-drag">
        {errorMessage && (
          <div
            className="status-panel status-panel--error mb-3 rounded-[18px] border border-red-500/25 bg-red-500/10 px-3 py-3 text-red-100"
            style={{ borderRadius: 18, overflow: "hidden" }}
          >
            <p className="text-[11px] leading-5 text-red-100/90">{errorMessage}</p>
          </div>
        )}
        <button
          onClick={() => saveSettings(settings)}
          className="w-full rounded-lg bg-primary py-2.5 text-xs font-bold text-slate-950 shadow-[0_14px_24px_rgba(209,122,40,0.18)] transition-all hover:bg-[#b86a1f] active:scale-[0.98]"
        >
          Save Changes
        </button>
      </div>
    </div>
    </SettingsShell>
  );
}
