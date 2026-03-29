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
import { Select } from "./components/Select";
import { FamVoiceLockup } from "./components/FamVoiceLockup";
import { buildHotkeyString, formatHotkey } from "./appHelpers";
import type { Replacement, SaveSettingsPayload, SettingsViewModel } from "./appTypes";

const controlMotion = "transition-colors duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)]";

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
      className="signal-shell signal-shell--settings relative flex h-full w-full min-h-0 flex-col overflow-hidden rounded-[20px]"
    >
      <div data-tauri-drag-region className="relative z-10 flex items-center justify-between px-4 py-2.5">
        <div className="flex items-center gap-2 pointer-events-none select-none">
          <FamVoiceLockup markSize={16} />
          <span className="ml-1.5 text-sm font-medium text-slate-300">Settings</span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className={`focus-ring rounded-full p-1.5 text-slate-500 ${controlMotion} hover:bg-white/10 hover:text-white`}
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
    <section className="control-section py-3 px-1">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <p className="section-eyebrow text-sm font-medium tracking-tight text-slate-200">
            {eyebrow}
          </p>
          {description ? (
            <p className="max-w-[42rem] text-base leading-7 text-slate-500">
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

  const loadSettings = async () => {
    setErrorMessage(null);
    setSettings(null);
    try {
      const loadedSettings = await invoke<SettingsViewModel>("get_settings");
      setSettings(toSettingsDraft(loadedSettings));
      setApiKeyInput("");
      setGroqApiKeyInput("");
    } catch (error) {
      console.error("Failed to load settings:", error);
      setErrorMessage(String(error));
    }
  };

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
    void loadSettings();

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
    <div className="flex items-center justify-between text-sm text-slate-300">
      <span>Current version</span>
      <span className="text-slate-400">{appVersion ? `v${appVersion}` : "Loading..."}</span>
    </div>
  );

  if (!settings) {
    return (
      <SettingsShell onClose={closeSettingsWindow}>
        <div className="flex h-full items-center justify-center px-4 py-4 no-drag">
          {errorMessage ? (
            <div className="max-w-md rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-4 text-left text-red-50">
              <p className="text-base font-medium text-red-100">
                Could not load settings.
              </p>
              <p className="mt-2 text-base leading-7 text-red-50/90">
                Retry loading the window. If it keeps failing, close it and open Settings again.
              </p>
              <div className="mt-4 flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void loadSettings()}
                  className={`focus-ring rounded-full border border-red-200/20 bg-black/20 px-3 py-1.5 text-sm font-medium text-red-50 ${controlMotion} hover:border-red-100/40 hover:text-white`}
                >
                  Retry loading settings
                </button>
                <button
                  type="button"
                  onClick={closeSettingsWindow}
                  className={`focus-ring rounded-full border border-white/10 bg-black/20 px-3 py-1.5 text-sm font-medium text-red-50/80 ${controlMotion} hover:border-white/20 hover:text-white`}
                >
                  Close window
                </button>
              </div>
              <p className="mt-3 text-sm leading-6 text-red-50/70">
                {errorMessage}
              </p>
            </div>
          ) : (
            <div className="max-w-md rounded-2xl border border-white/10 bg-black/20 px-4 py-4 text-left text-slate-200">
              <p className="text-base font-medium tracking-tight text-slate-200">
                Loading
              </p>
              <p className="mt-2 text-base leading-7 text-slate-100">
                Loading settings...
              </p>
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
          <label className="flex flex-col gap-1.5 text-sm text-slate-300">
            Provider
            <Select
              value={settings.transcription_provider}
              onChange={(provider) => {
                const models = MODELS_BY_PROVIDER[provider] ?? [];
                setSettings({
                  ...settings,
                  transcription_provider: provider,
                  model: models[0]?.value ?? "",
                });
              }}
              options={TRANSCRIPTION_PROVIDERS}
            />
          </label>

          <label className="flex flex-col gap-1.5 text-sm text-slate-300">
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
                className={`focus-ring w-full border-b border-white/10 bg-transparent p-1.5 text-sm text-white ${controlMotion} focus-visible:border-primary`}
                placeholder={settings.api_key_masked ?? "sk-..."}
              />
              <span className="text-base leading-7 text-slate-400">
                {settings.api_key_present
                  ? `Saved in your OS credential store as ${settings.api_key_masked}. Leave blank to keep it.`
                : "Used for OpenAI transcription and prompt optimization. Saved after you enter one."}
            </span>
          </label>

          {settings.transcription_provider === "groq" && (
            <label className="flex flex-col gap-1.5 text-sm text-slate-300">
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
                className={`focus-ring w-full border-b border-white/10 bg-transparent p-1.5 text-sm text-white ${controlMotion} focus-visible:border-primary`}
                placeholder={settings.groq_api_key_masked ?? "gsk_..."}
              />
              <span className="text-base leading-7 text-slate-400">
                {settings.groq_api_key_present
                  ? `Saved in your OS credential store as ${settings.groq_api_key_masked}. Leave blank to keep it.`
                  : "Saved in your OS credential store after you enter one."}
              </span>
            </label>
          )}

          <label className="flex flex-col gap-1.5 text-sm text-slate-300">
            Model
            <Select
              value={settings.model}
              onChange={(value) => setSettings({ ...settings, model: value })}
              options={MODELS_BY_PROVIDER[settings.transcription_provider] ?? []}
            />
          </label>
          </ControlSection>

          <ControlSection
            eyebrow="Prompt Optimization"
            description="Runs a second OpenAI pass after transcription to rewrite the finalized transcript into an English implementation prompt for coding agents."
          >

          <label className={`flex items-center gap-3 text-sm text-slate-300 cursor-pointer ${controlMotion} hover:text-white`}>
            <input
              type="checkbox"
              checked={settings.prompt_optimization_enabled}
              onChange={(e) => setSettings({ ...settings, prompt_optimization_enabled: e.target.checked })}
              className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
            />
            <div className="flex flex-col">
              <span>Improve into prompt</span>
              <span className="text-base leading-7 text-slate-400">Adds an extra OpenAI model pass that rewrites the finalized transcript into an English implementation prompt for a coding agent.</span>
            </div>
          </label>

          <label className="flex flex-col gap-1.5 text-sm text-slate-300">
            Model
            <Select
              value={settings.prompt_optimizer_model}
              onChange={(value) => setSettings({ ...settings, prompt_optimizer_model: value })}
              options={PROMPT_OPTIMIZER_MODELS}
            />
          </label>

          <p className="text-base leading-7 text-slate-400">
            Uses the saved OpenAI API key above. Keep the static metaprompt first and the dictated request last to maximize prompt caching.
          </p>
          </ControlSection>

          <ControlSection eyebrow="Behavior">
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5 text-sm text-slate-300">
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
                  className={`focus-ring flex-1 w-full cursor-pointer border-b bg-transparent p-1.5 text-sm text-white ${controlMotion} focus-visible:border-primary ${isListening ? "border-primary text-primary" : "border-white/10"}`}
                />
                <button
                  type="button"
                  onClick={() => setSettings({ ...settings, hotkey: DEFAULT_HOTKEY })}
                  aria-label="Reset hotkey to default"
                  className={`focus-ring cursor-pointer rounded border border-white/10 bg-black/40 p-2 text-gray-400 ${controlMotion} hover:border-primary/50 hover:text-primary`}
                >
                  <RefreshCw size={12} />
                </button>
              </div>
            </div>
            <label className="flex flex-col gap-1.5 text-sm text-slate-300">
              Language Preference
              <Select
                value={settings.language}
                onChange={(value) => setSettings({ ...settings, language: value })}
                options={LANGUAGES}
              />
              <span className="text-base leading-7 text-slate-400">
                Auto Detect handles mixed dictation. Choose Portuguese or English only if you want to bias transcription toward one language.
              </span>
            </label>

            <label className="flex flex-col gap-1.5 text-sm text-slate-300">
              Mic Sensitivity
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={settings.mic_sensitivity}
                  onChange={(e) => setSettings({ ...settings, mic_sensitivity: Number(e.target.value) })}
                  className="focus-ring flex-1 cursor-pointer accent-primary"
                />
                <span className="w-8 text-right text-sm text-slate-400">
                  {settings.mic_sensitivity}
                </span>
              </div>
              <div className="flex justify-between text-sm text-slate-500">
                <span>Less noise</span>
                <span>Quieter voice</span>
              </div>
              <span className="text-base leading-7 text-slate-400">
                Higher sensitivity helps softer speech, but can pick up more background noise.
              </span>
            </label>
          </div>

          <div className="space-y-2">
            <label className={`flex items-center gap-3 text-sm text-slate-300 cursor-pointer ${controlMotion} hover:text-white`}>
              <input
                type="checkbox"
                checked={settings.widget_mode}
                onChange={(e) => setSettings({ ...settings, widget_mode: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Widget Mode</span>
                <span className="text-base leading-7 text-slate-400">Minimal UI with only waveforms</span>
              </div>
            </label>

            <label className={`flex items-center gap-3 text-sm text-slate-300 cursor-pointer ${controlMotion} hover:text-white`}>
              <input
                type="checkbox"
                checked={settings.auto_paste}
                onChange={(e) => setSettings({ ...settings, auto_paste: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <span>Auto Paste Transcript</span>
            </label>

            <label className={`flex items-center gap-3 text-sm text-slate-300 cursor-pointer ${controlMotion} hover:text-white`}>
              <input
                type="checkbox"
                checked={settings.preserve_clipboard}
                onChange={(e) => setSettings({ ...settings, preserve_clipboard: e.target.checked })}
                className="w-4 h-4 rounded border-white/10 bg-black/40 accent-primary cursor-pointer"
              />
              <div className="flex flex-col">
                <span>Preserve Clipboard</span>
                <span className="text-base leading-7 text-slate-400">Restore the original clipboard after a successful auto-paste</span>
              </div>
            </label>

            <label className={`flex items-center gap-3 text-sm text-slate-300 cursor-pointer ${controlMotion} hover:text-white`}>
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
              <p className="pl-7 text-base leading-7 text-slate-400">
                Launch on Startup is only available from the installed app.
              </p>
            )}
          </div>
          </ControlSection>

          <ControlSection
            eyebrow="Update"
            action={
              <button
                type="button"
                onClick={() => void refreshUpdate()}
                className={`focus-ring flex cursor-pointer items-center gap-1 text-sm text-slate-400 ${controlMotion} hover:text-primary`}
              >
                <RefreshCw size={10} />
                Refresh
              </button>
            }
          >
            <div className="space-y-3">
              {isCheckingForUpdates ? (
                <div className="py-2 text-slate-200">
                  {currentVersionRow}
                  <p className="mt-3 text-base leading-7 text-slate-400">
                    Checking for updates...
                  </p>
                </div>
              ) : updateCheckError ? (
                <div className="py-2 text-red-400">
                  {currentVersionRow}
                  <p className="mt-3 text-base font-medium text-red-100">
                    Could not check for updates.
                  </p>
                  <p className="mt-1 text-base leading-7 text-red-100/90">
                    Refresh to try again.
                  </p>
                  <p className="mt-2 text-sm leading-6 text-red-100/70">
                    {updateCheckError}
                  </p>
                </div>
              ) : availableUpdate ? (
                <div
                  className="py-2 text-slate-200"
                >
                  {currentVersionRow}
                  <div className="mt-3 flex items-center justify-between text-sm text-slate-300">
                    <span>Update available</span>
                    <span className="text-sm font-mono text-slate-400">v{availableUpdate.version}</span>
                  </div>
                  <button
                    type="button"
                    onClick={handleApplyUpdate}
                    disabled={isApplyingUpdate}
                    className="focus-ring mt-3 w-full rounded-lg border border-primary/30 bg-primary/10 py-2 text-sm font-semibold text-amber-50 transition-[background-color,color,transform] duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)] hover:bg-primary/20 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {isApplyingUpdate ? "Updating..." : "Update"}
                  </button>
                </div>
              ) : (
                <div className="py-2 text-slate-200">
                  {currentVersionRow}
                  <p className="mt-3 text-base leading-7 text-slate-400">
                    No update available.
                  </p>
                </div>
              )}

              {updateInstallError && (
                <div className="py-2 text-red-400">
                  <p className="text-base font-medium text-red-100">
                    Update installation failed.
                  </p>
                  <p className="mt-1 text-base leading-7 text-red-100/90">
                    Close and reopen FamVoice, then try installing the update again.
                  </p>
                  <p className="mt-2 text-sm leading-6 text-red-100/70">
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
                type="button"
                onClick={addReplacement}
                className={`focus-ring flex cursor-pointer items-center gap-1 text-sm text-primary ${controlMotion} hover:text-amber-200`}
              >
                <Plus size={10} /> Add
              </button>
            }
          >
            <div className="space-y-2">
              {settings.replacements.map((replacement) => (
                <div
                  key={replacement.id}
                  className="flex flex-col gap-3 rounded-xl border border-white/10 bg-black/20 p-3 md:flex-row md:items-end"
                >
                  <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm font-medium text-slate-400">
                    <span>Spoken term</span>
                    <input
                      value={replacement.target}
                      onChange={(e) => updateReplacement(replacement.id, "target", e.target.value)}
                      className={`focus-ring min-w-0 border-b border-white/10 bg-transparent p-1.5 text-base text-white ${controlMotion} focus-visible:border-primary`}
                    />
                  </label>
                  <span className="hidden pb-2 text-sm text-slate-400 md:block">-&gt;</span>
                  <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm font-medium text-slate-400">
                    <span>Replacement</span>
                    <input
                      value={replacement.replacement}
                      onChange={(e) => updateReplacement(replacement.id, "replacement", e.target.value)}
                      className={`focus-ring min-w-0 border-b border-white/10 bg-transparent p-1.5 text-base text-white ${controlMotion} focus-visible:border-primary`}
                    />
                  </label>
                  <button
                    type="button"
                    onClick={() => removeReplacement(replacement.id)}
                    aria-label="Delete glossary row"
                    className={`focus-ring self-end cursor-pointer rounded p-1 text-gray-600 ${controlMotion} hover:text-red-400`}
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
              {settings.replacements.length === 0 && (
                <p className="text-base italic text-slate-500">No glossary entries configured.</p>
              )}
            </div>
          </ControlSection>
        </div>
      </div>

      <div className="px-4 py-3 no-drag">
        {errorMessage && (
          <div className="mb-3 rounded-xl border border-red-500/20 bg-red-500/10 px-3 py-3 text-red-50">
            <p className="text-base font-medium text-red-100">
              Could not save settings.
            </p>
            <p className="mt-1 text-base leading-7 text-red-50/90">
              Check the fields above, fix any missing values, and save again.
            </p>
            <p className="mt-2 text-sm leading-6 text-red-50/70">
              {errorMessage}
            </p>
          </div>
        )}
        <button
          type="button"
          onClick={() => saveSettings(settings)}
          className="focus-ring w-full rounded bg-primary py-2.5 text-sm font-semibold text-slate-950 transition-[background-color,color,transform] duration-[var(--fam-duration-fast)] ease-[var(--fam-ease-ease)] hover:bg-[#b86a1f] active:scale-[0.98]"
        >
          Save changes
        </button>
      </div>
    </div>
    </SettingsShell>
  );
}
