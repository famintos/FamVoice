import assert from "node:assert/strict";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, test, vi } from "vitest";
import { SettingsView } from "./SettingsView";
import {
  makeSettings,
  makeUpdate,
  resetTauriMocks,
  setInvokeHandler,
  tauriMocks,
} from "./test/tauriMocks";

beforeEach(() => {
  resetTauriMocks();
});

test("marks gpt-transcribe as the recommended OpenAI default", async () => {
  resetTauriMocks({
    settings: makeSettings({
      transcription_provider: "openai",
      model: "gpt-transcribe",
    }),
  });

  render(<SettingsView />);

  const model = await screen.findByRole("combobox", { name: "Transcription model" });
  assert.equal((model as HTMLSelectElement).value, "gpt-transcribe");
  assert.ok(screen.getByRole("option", { name: "gpt-transcribe — Recommended" }));
  assert.ok(screen.getByRole("option", { name: "whisper-1 — Specialized fallback" }));
  assert.ok(screen.getByText(/Recommended for completed dictation/));
});

test("preserves an existing explicit OpenAI whisper-1 choice", async () => {
  resetTauriMocks({
    settings: makeSettings({
      transcription_provider: "openai",
      model: "whisper-1",
    }),
  });

  render(<SettingsView />);

  const model = await screen.findByRole("combobox", { name: "Transcription model" });
  assert.equal((model as HTMLSelectElement).value, "whisper-1");
  assert.ok(screen.getByText(/Specialized fallback for word timestamps/));
});

test("shows the sanitized one-time transcription migration notice", async () => {
  const notice = "FamVoice updated your legacy OpenAI transcription model to gpt-transcribe.";
  resetTauriMocks({
    settings: makeSettings({
      transcription_provider: "openai",
      model: "gpt-transcribe",
      transcription_model_notice: notice,
    }),
  });

  render(<SettingsView />);

  assert.ok(await screen.findByText("Transcription model updated"));
  assert.ok(screen.getByText(notice));
});

test("uses the explicit provider default and saves the switched OpenAI model", async () => {
  resetTauriMocks({
    settings: makeSettings({
      transcription_provider: "groq",
      model: "whisper-large-v3",
    }),
  });
  const user = userEvent.setup();

  render(<SettingsView />);
  const provider = await screen.findByRole("combobox", { name: "Provider" });
  const model = screen.getByRole("combobox", { name: "Transcription model" });

  await user.selectOptions(provider, "openai");
  assert.equal((model as HTMLSelectElement).value, "gpt-transcribe");
  await user.click(screen.getByRole("button", { name: "Save changes" }));

  await waitFor(() => {
    const saveCall = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(saveCall);
    const args = saveCall[1] as {
      newSettings: { transcription_provider: string; model: string };
    };
    assert.equal(args.newSettings.transcription_provider, "openai");
    assert.equal(args.newSettings.model, "gpt-transcribe");
  });
});

test("captures keyboard and side-mouse recording hotkeys", async () => {
  render(<SettingsView />);

  const hotkey = await screen.findByRole("textbox", { name: "Recording hotkey" });
  fireEvent.focus(hotkey);
  assert.equal((hotkey as HTMLInputElement).value, "Ctrl + Shift + Space");
  fireEvent.keyDown(hotkey, { key: "Enter" });
  await waitFor(() => assert.equal(
    (hotkey as HTMLInputElement).value,
    "Capturing... Escape to cancel",
  ));
  fireEvent.keyDown(hotkey, { key: "K", ctrlKey: true, shiftKey: true });
  assert.equal((hotkey as HTMLInputElement).value, "Ctrl + Shift + K");

  fireEvent.focus(hotkey);
  fireEvent.click(hotkey);
  await waitFor(() => assert.equal(
    (hotkey as HTMLInputElement).value,
    "Capturing... Escape to cancel",
  ));
  fireEvent.mouseDown(hotkey, { button: 3 });
  assert.equal((hotkey as HTMLInputElement).value, "Mouse 4 (Back)");
});

test("shows a recoverable error when saving settings fails", async () => {
  const expectedError = "simulated settings persistence failure";
  setInvokeHandler("save_settings", () => Promise.reject(new Error(expectedError)));
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
  const user = userEvent.setup();

  render(<SettingsView />);
  await user.click(await screen.findByRole("button", { name: "Save changes" }));

  assert.ok(await screen.findByText("Could not save settings."));
  assert.ok(screen.getByText(new RegExp(expectedError)));
  assert.equal(
    tauriMocks.invoke.mock.calls.some(([command]) => command === "close_settings_window"),
    false,
  );
  consoleError.mockRestore();
});

test("renders and applies an available update", async () => {
  const update = makeUpdate("0.4.0");
  tauriMocks.check.mockResolvedValue(update);
  const user = userEvent.setup();

  render(<SettingsView />);
  assert.ok(await screen.findByText("Update available"));
  assert.ok(screen.getByText("v0.4.0"));
  await user.click(screen.getByRole("button", { name: "Update" }));

  await waitFor(() => assert.equal(update.downloadAndInstall.mock.calls.length, 1));
  assert.equal(tauriMocks.relaunch.mock.calls.length, 1);
});

test("renders updater failures and allows a new check", async () => {
  tauriMocks.check.mockRejectedValueOnce(new Error("offline"));
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
  const user = userEvent.setup();

  render(<SettingsView />);
  assert.ok(await screen.findByText("Could not check for updates."));

  tauriMocks.check.mockResolvedValue(null);
  await user.click(screen.getByRole("button", { name: "Refresh" }));
  assert.ok(await screen.findByText("No update available."));
  consoleError.mockRestore();
});

test("runs content-free diagnostics and exports the sanitized report", async () => {
  setInvokeHandler("run_microphone_test", () => ({
    status: "ok",
    rms: 0.2,
    peak: 0.5,
    signalDetected: true,
    sampleCount: 16_000,
  }));
  setInvokeHandler("test_provider_auth", () => ({
    status: "ok",
    provider: "Groq",
    latencyMs: 42,
    authenticated: true,
    error: null,
  }));
  setInvokeHandler("export_diagnostics", () => "C:\\Downloads\\famvoice-diagnostics.json");
  const user = userEvent.setup();
  render(<SettingsView />);

  await user.click(await screen.findByRole("button", { name: "Test microphone" }));
  await user.click(screen.getByRole("button", { name: "Test provider" }));
  await user.click(screen.getByRole("button", { name: "Export report" }));

  assert.ok(tauriMocks.invoke.mock.calls.some(([command]) => command === "run_microphone_test"));
  assert.ok(tauriMocks.invoke.mock.calls.some(([command]) => command === "test_provider_auth"));
  assert.ok(tauriMocks.invoke.mock.calls.some(([command]) => command === "export_diagnostics"));
});

test("applies an explicit bounded history retention policy", async () => {
  setInvokeHandler("set_history_retention", (args) => ({
    maxItems: (args as { maxItems: number }).maxItems,
  }));
  const user = userEvent.setup();
  render(<SettingsView />);

  const retention = await screen.findByRole("combobox", { name: "Local transcript retention" });
  await user.selectOptions(retention, "25");
  await user.click(screen.getByRole("button", { name: "Apply retention" }));

  assert.ok(tauriMocks.invoke.mock.calls.some(([command, args]) =>
    command === "set_history_retention" && (args as { maxItems: number }).maxItems === 25));
});
