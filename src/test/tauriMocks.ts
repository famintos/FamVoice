import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import type { HistoryItem, SettingsViewModel, Status } from "../appTypes";

type EventListener = (event: { payload: unknown }) => void;
type InvokeHandler = (args: unknown) => unknown | Promise<unknown>;
type WindowEventListener = (event: { payload: unknown }) => void;

export interface MockUpdate {
  version: string;
  downloadAndInstall: ReturnType<typeof vi.fn>;
}

const mocks = vi.hoisted(() => {
  const commandHandlers = new Map<string, InvokeHandler>();
  const eventListeners = new Map<string, Set<EventListener>>();
  const windowEventListeners = new Map<string, Set<WindowEventListener>>();

  const invoke = vi.fn(async (command: string, args?: unknown): Promise<unknown> => {
    const handler = commandHandlers.get(command);
    return handler ? handler(args) : undefined;
  });

  const listen = vi.fn(async (event: string, listener: EventListener) => {
    const listeners = eventListeners.get(event) ?? new Set<EventListener>();
    listeners.add(listener);
    eventListeners.set(event, listeners);
    return () => listeners.delete(listener);
  });

  const addWindowListener = async (event: string, listener: WindowEventListener) => {
    const listeners = windowEventListeners.get(event) ?? new Set<WindowEventListener>();
    listeners.add(listener);
    windowEventListeners.set(event, listeners);
    return () => listeners.delete(listener);
  };

  return {
    commandHandlers,
    eventListeners,
    windowEventListeners,
    invoke,
    listen,
    check: vi.fn<() => Promise<MockUpdate | null>>(),
    getVersion: vi.fn<() => Promise<string>>(),
    isEnabled: vi.fn<() => Promise<boolean>>(),
    enable: vi.fn<() => Promise<void>>(),
    disable: vi.fn<() => Promise<void>>(),
    relaunch: vi.fn<() => Promise<void>>(),
    cursorPosition: vi.fn<() => Promise<{ x: number; y: number }>>(),
    window: {
      minimize: vi.fn<() => Promise<void>>(),
      hide: vi.fn<() => Promise<void>>(),
      setIgnoreCursorEvents: vi.fn<(_ignore: boolean) => Promise<void>>(),
      innerPosition: vi.fn<() => Promise<{ x: number; y: number }>>(),
      scaleFactor: vi.fn<() => Promise<number>>(),
      startDragging: vi.fn<() => Promise<void>>(),
      onMoved: vi.fn((listener: WindowEventListener) => addWindowListener("moved", listener)),
      onScaleChanged: vi.fn((listener: WindowEventListener) =>
        addWindowListener("scale-changed", listener)),
      onFocusChanged: vi.fn((listener: WindowEventListener) =>
        addWindowListener("focus-changed", listener)),
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mocks.getVersion }));
vi.mock("@tauri-apps/api/window", () => ({
  cursorPosition: mocks.cursorPosition,
  getCurrentWindow: () => mocks.window,
}));
vi.mock("@tauri-apps/plugin-autostart", () => ({
  disable: mocks.disable,
  enable: mocks.enable,
  isEnabled: mocks.isEnabled,
}));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));

export const tauriMocks = mocks;

export function makeSettings(
  overrides: Partial<SettingsViewModel> = {},
): SettingsViewModel {
  return {
    transcription_provider: "groq",
    api_key_present: true,
    api_key_masked: "sk-...test",
    groq_api_key_present: true,
    groq_api_key_masked: "gsk_...test",
    model: "whisper-large-v3",
    language: "pt",
    auto_paste: true,
    preserve_clipboard: true,
    hotkey: "CommandOrControl+Shift+Space",
    input_device_id: "",
    repaste_hotkey: "",
    noise_suppression_enabled: true,
    widget_mode: false,
    mic_sensitivity: 50,
    prompt_optimization_enabled: false,
    prompt_optimizer_model: "gpt-5-mini",
    replacements: [],
    credential_storage: {
      mode: "secure_store",
      message: null,
    },
    transcription_model_notice: null,
    ...overrides,
  };
}

export function makeUpdate(version = "0.4.0"): MockUpdate {
  return {
    version,
    downloadAndInstall: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  };
}

export function resetTauriMocks({
  settings = makeSettings(),
  history = [],
}: {
  settings?: SettingsViewModel;
  history?: HistoryItem[];
} = {}): void {
  vi.clearAllMocks();
  mocks.commandHandlers.clear();
  mocks.eventListeners.clear();
  mocks.windowEventListeners.clear();

  mocks.commandHandlers.set("get_settings", () => settings);
  mocks.commandHandlers.set("get_history", () => history);
  mocks.commandHandlers.set("get_retry_audio_state", () => ({ available: false }));
  mocks.commandHandlers.set("get_history_retention", () => ({ maxItems: 100 }));
  mocks.commandHandlers.set("get_diagnostics_snapshot", () => ({
    version: { appVersion: "0.3.29", platform: "windows", architecture: "x86_64" },
    device: {
      status: "ok",
      selectedLabel: "System default microphone",
      usesSystemDefault: true,
      connected: true,
      streamHealthy: true,
    },
    hotkey: {
      status: "ok",
      recordingHotkey: "CommandOrControl+Shift+Space",
      recordingAvailable: true,
      repasteHotkey: null,
      repasteAvailable: null,
      conflict: false,
    },
    provider: {
      status: "warning",
      provider: "Groq",
      model: "whisper-large-v3",
      apiKeyConfigured: true,
      lastTest: null,
    },
    microphoneTest: null,
    lastOperation: null,
  }));
  mocks.commandHandlers.set("list_input_devices", () => []);
  mocks.commandHandlers.set("get_dictation_activity", () => ({
    active: false,
    recording: false,
    transcribing: false,
  }));
  mocks.commandHandlers.set("can_manage_autostart", () => true);
  mocks.commandHandlers.set("save_settings", (args) => {
    const payload = (args as { newSettings: SettingsViewModel }).newSettings;
    return { ...settings, ...payload };
  });

  mocks.check.mockResolvedValue(null);
  mocks.getVersion.mockResolvedValue("0.3.29");
  mocks.isEnabled.mockResolvedValue(false);
  mocks.enable.mockResolvedValue(undefined);
  mocks.disable.mockResolvedValue(undefined);
  mocks.relaunch.mockResolvedValue(undefined);
  mocks.cursorPosition.mockResolvedValue({ x: 0, y: 0 });
  mocks.window.minimize.mockResolvedValue(undefined);
  mocks.window.hide.mockResolvedValue(undefined);
  mocks.window.setIgnoreCursorEvents.mockResolvedValue(undefined);
  mocks.window.innerPosition.mockResolvedValue({ x: 0, y: 0 });
  mocks.window.scaleFactor.mockResolvedValue(1);
  mocks.window.startDragging.mockResolvedValue(undefined);
}

export function setInvokeHandler(command: string, handler: InvokeHandler): void {
  mocks.commandHandlers.set(command, handler);
}

export function emitTauriEvent<T>(event: string, payload: T): void {
  for (const listener of mocks.eventListeners.get(event) ?? []) {
    listener({ payload });
  }
}

export function emitStatus(status: Status): void {
  emitTauriEvent("status", status);
}

class ResizeObserverMock {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

Object.defineProperty(globalThis, "ResizeObserver", {
  configurable: true,
  value: ResizeObserverMock,
});

Object.defineProperty(navigator, "clipboard", {
  configurable: true,
  value: { writeText: vi.fn<(_text: string) => Promise<void>>().mockResolvedValue(undefined) },
});

Object.defineProperty(window, "requestAnimationFrame", {
  configurable: true,
  value: (callback: FrameRequestCallback) => window.setTimeout(() => callback(performance.now()), 0),
});

Object.defineProperty(window, "cancelAnimationFrame", {
  configurable: true,
  value: (handle: number) => window.clearTimeout(handle),
});

afterEach(() => {
  cleanup();
});
