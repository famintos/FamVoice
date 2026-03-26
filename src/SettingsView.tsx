import { useEffect, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
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

  const refreshUpdate = async () => {
    setIsCheckingForUpdates(true);
    setUpdateCheckError(null);
    try {
      const update = await check();
      setAvailableUpdate(update);
      setUpdateCheckError(null);
    } catch (error) {
      console.error("Update check failed:", error);
      setAvailableUpdate(null);
      setUpdateCheckError(String(error));
    } finally {
      setIsCheckingForUpdates(false);
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

  if (!settings) {
    return (
      <main className="w-full h-full flex items-center justify-center bg-[#0f0f13] px-4 text-center text-xs">
        <div className={errorMessage ? "text-red-300" : "text-gray-400"}>
          {errorMessage ?? "Loading settings..."}
        </div>
      </main>
    );
  }

  return (
    <main data-tauri-drag-region className="w-full h-full flex flex-col p-4 bg-[#0f0f13] text-white overflow-hidden border border-white/10 rounded-xl">
      <div
        className="-mx-4 -mt-4 mb-2 px-4 pt-4 pb-3 select-none"
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 pointer-events-none">
            <SettingsIcon size={14} className="text-primary" />
            <h2 className="text-sm font-bold tracking-wide">Settings</h2>
          </div>
          <button onClick={() => invoke("close_settings_window")} className="p-1 hover:bg-white/10 rounded cursor-pointer transition-colors">
            <X size={16} className="text-gray-400 hover:text-white" />
          </button>
        </div>
      </div>

      <div className="flex-1 flex flex-col gap-5 overflow-y-auto overflow-x-hidden pr-2 custom-scrollbar pb-4 no-drag">
        <section className="space-y-3">
          <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Transcription</h3>
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
              <span className={`flex items-center gap-1 text-[10px] font-medium ${settings.api_key_present ? "text-green-400" : "text-amber-400"}`}>
                <span className={`w-1.5 h-1.5 rounded-full ${settings.api_key_present ? "bg-green-400" : "bg-amber-400"}`} />
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
                <span className={`flex items-center gap-1 text-[10px] font-medium ${settings.groq_api_key_present ? "text-green-400" : "text-amber-400"}`}>
                  <span className={`w-1.5 h-1.5 rounded-full ${settings.groq_api_key_present ? "bg-green-400" : "bg-amber-400"}`} />
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
        </section>

        <section className="space-y-3">
          <div className="flex flex-col gap-1">
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Prompt Optimization</h3>
            <span className="text-[10px] text-gray-500">
              Runs a second OpenAI pass after transcription to rewrite the finalized transcript into an English implementation prompt for coding agents.
            </span>
          </div>

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
        </section>

        <section className="space-y-3">
          <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Behavior</h3>
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
        </section>

        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Update</h3>
            <button
              onClick={() => void refreshUpdate()}
              className="text-[10px] text-gray-400 hover:text-white flex items-center gap-1 transition-colors cursor-pointer"
              type="button"
            >
              <RefreshCw size={10} />
              Refresh
            </button>
          </div>

          <div className="rounded-xl border border-white/10 bg-black/30 p-3 space-y-3">
            <div className="flex items-center justify-between text-xs text-gray-300">
              <span>Current version</span>
              <span className="text-gray-400">{appVersion ? `v${appVersion}` : "Loading..."}</span>
            </div>

            {isCheckingForUpdates ? (
              <p className="text-[11px] text-gray-400">Checking for updates...</p>
            ) : updateCheckError ? (
              <div className="space-y-2">
                <p className="text-[11px] text-red-200">Unable to check for updates right now.</p>
                <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
                  {updateCheckError}
                </div>
              </div>
            ) : availableUpdate ? (
              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs text-gray-300">
                  <span>Update available</span>
                  <span className="text-sky-300">v{availableUpdate.version}</span>
                </div>
                <button
                  type="button"
                  onClick={handleApplyUpdate}
                  disabled={isApplyingUpdate}
                  className="w-full rounded-lg border border-sky-400/30 bg-sky-500/10 py-2 text-xs font-bold text-sky-200 transition-colors hover:bg-sky-500/20 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {isApplyingUpdate ? "Updating..." : "Update"}
                </button>
              </div>
            ) : (
              <p className="text-[11px] text-gray-400">No update available.</p>
            )}

            {updateInstallError && (
              <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
                {updateInstallError}
              </div>
            )}
          </div>
        </section>

        <section className="space-y-3">
          <div className="flex justify-between items-center">
            <h3 className="text-[10px] uppercase font-bold text-gray-500 tracking-wider">Glossary</h3>
            <button
              onClick={addReplacement}
              className="text-[10px] text-primary hover:text-blue-400 flex items-center gap-1 cursor-pointer"
            >
              <Plus size={10} /> Add
            </button>
          </div>

          <div className="space-y-2">
            {settings.replacements.map((replacement) => (
              <div key={replacement.id} className="flex gap-2 items-center">
                <input
                  value={replacement.target}
                  onChange={(e) => updateReplacement(replacement.id, "target", e.target.value)}
                  placeholder="spoken term"
                  className="flex-1 min-w-0 p-1.5 bg-black/40 rounded border border-white/10 text-[10px] text-white focus:outline-none focus:border-primary transition-colors"
                />
                <span className="text-gray-500 text-[10px]">-&gt;</span>
                <input
                  value={replacement.replacement}
                  onChange={(e) => updateReplacement(replacement.id, "replacement", e.target.value)}
                  placeholder="preferred text"
                  className="flex-1 min-w-0 p-1.5 bg-black/40 rounded border border-white/10 text-[10px] text-white focus:outline-none focus:border-primary transition-colors"
                />
                <button
                  onClick={() => removeReplacement(replacement.id)}
                  className="text-gray-600 hover:text-red-400 p-1 cursor-pointer"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
            {settings.replacements.length === 0 && (
              <p className="text-[10px] text-gray-600 italic">No glossary entries configured.</p>
            )}
          </div>
        </section>
      </div>

      <div className="pt-4 border-t border-white/5 mt-auto">
        {errorMessage && (
          <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
            {errorMessage}
          </div>
        )}
        <button
          onClick={() => saveSettings(settings)}
          className="w-full bg-primary hover:bg-blue-600 py-2.5 rounded text-xs font-bold cursor-pointer transition-all shadow-lg active:scale-[0.98]"
        >
          Save Changes
        </button>
      </div>
    </main>
  );
}
