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

test("saves OpenRouter MAI, Portuguese and Mouse 5 with optimization disabled", async () => {
  resetTauriMocks({ settings: makeSettings({ language: "en", prompt_optimization_enabled: true }) });
  const user = userEvent.setup();
  render(<SettingsView />);
  await user.selectOptions(await screen.findByRole("combobox", { name: "Provider" }), "openrouter");
  assert.equal((screen.getByRole("combobox", { name: "Transcription model" }) as HTMLSelectElement).value, "microsoft/mai-transcribe-2");
  assert.ok(screen.getByRole("option", { name: "MAI Transcribe 2" }));
  assert.equal(screen.queryByLabelText(/Improve into prompt/), null);
  assert.equal(screen.queryByRole("combobox", { name: "Model" }), null);
  assert.equal(screen.queryByText("OpenAI API Key"), null);
  assert.ok(screen.getByText(/Inserts text without sending it/));
  const input = screen.getByLabelText("OpenRouter API Key") as HTMLInputElement;
  assert.equal(input.type, "password");
  assert.equal(input.autocomplete, "off");
  await user.type(input, "  synthetic-openrouter-key  ");
  await user.selectOptions(screen.getByRole("combobox", { name: /Language Preference/ }), "pt");
  const hotkey = screen.getByRole("textbox", { name: "Recording hotkey" });
  fireEvent.click(hotkey);
  fireEvent.mouseDown(hotkey, { button: 4 });
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.transcription_provider, "openrouter");
    assert.equal(newSettings.model, "microsoft/mai-transcribe-2");
    assert.equal(newSettings.openrouter_api_key, "synthetic-openrouter-key");
    assert.equal(newSettings.api_key, null);
    assert.equal(newSettings.groq_api_key, null);
    assert.equal(newSettings.soniox_api_key, null);
    assert.equal(newSettings.language, "pt");
    assert.equal(newSettings.hotkey, "Mouse5");
    assert.equal(newSettings.prompt_optimization_enabled, false);
    assert.equal("openrouter_api_key_masked" in newSettings, false);
    assert.equal("openrouter_api_key_present" in newSettings, false);
  });
});

test("selects Scribe v2 using the saved OpenRouter key", async () => {
  resetTauriMocks({ settings: makeSettings({
    transcription_provider: "openrouter", model: "microsoft/mai-transcribe-2",
    openrouter_api_key_present: true, openrouter_api_key_masked: "synthetic-mask",
  }) });
  const user = userEvent.setup();
  render(<SettingsView />);
  const models = await screen.findByRole("combobox", { name: "Transcription model" });
  await user.selectOptions(models, "elevenlabs/scribe-v2");
  assert.ok(screen.getByRole("option", { name: "MAI Transcribe 2" }));
  assert.ok(screen.getByText(/Try Auto Detect for mixed speech/));
  await user.selectOptions(screen.getByRole("combobox", { name: /Language Preference/ }), "auto");
  assert.equal(screen.queryByLabelText(/Improve into prompt/), null);
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.transcription_provider, "openrouter");
    assert.equal(newSettings.model, "elevenlabs/scribe-v2");
    assert.equal(newSettings.openrouter_api_key, null);
    assert.equal(newSettings.language, "auto");
    assert.equal(newSettings.prompt_optimization_enabled, false);
  });
});

test.each(["", "   "])("keeps a saved OpenRouter key when the input is %j", async (draft) => {
  resetTauriMocks({ settings: makeSettings({
    transcription_provider: "openrouter", model: "microsoft/mai-transcribe-2",
    openrouter_api_key_present: true, openrouter_api_key_masked: "synthetic-mask",
    prompt_optimization_enabled: true,
  }) });
  const user = userEvent.setup();
  render(<SettingsView />);
  const input = await screen.findByLabelText("OpenRouter API Key") as HTMLInputElement;
  assert.equal(input.value, "");
  assert.equal(input.placeholder, "Saved OpenRouter key");
  assert.ok(screen.getByText("Configured"));
  assert.ok(screen.getByText("Key saved securely. Leave blank to keep it."));
  assert.equal(screen.queryByText(/synthetic-mask/), null);
  assert.equal(screen.queryByLabelText(/Improve into prompt/), null);
  if (draft) await user.type(input, draft);
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.openrouter_api_key, null);
    assert.equal(newSettings.prompt_optimization_enabled, false);
  });
});

test("retains provider key drafts and the existing optimization choice across provider switches", async () => {
  resetTauriMocks({ settings: makeSettings({ prompt_optimization_enabled: true }) });
  const user = userEvent.setup();
  render(<SettingsView />);
  const provider = await screen.findByRole("combobox", { name: "Provider" });
  const openaiInput = screen.getByPlaceholderText("sk-...test");
  await user.type(openaiInput, "synthetic-openai-key");
  await user.type(screen.getByPlaceholderText("gsk_...test"), "synthetic-groq-key");
  await user.selectOptions(provider, "soniox");
  await user.type(screen.getByLabelText("Soniox API Key"), "synthetic-soniox-key");
  await user.selectOptions(provider, "openrouter");
  await user.type(screen.getByLabelText("OpenRouter API Key"), "synthetic-openrouter-key");
  await user.selectOptions(provider, "groq");
  assert.equal((screen.getByLabelText(/Improve into prompt/) as HTMLInputElement).checked, true);
  assert.equal((screen.getByPlaceholderText("sk-...test") as HTMLInputElement).value, "synthetic-openai-key");
  assert.equal((screen.getByPlaceholderText("gsk_...test") as HTMLInputElement).value, "synthetic-groq-key");
  await user.selectOptions(provider, "openrouter");
  assert.equal((screen.getByLabelText("OpenRouter API Key") as HTMLInputElement).value, "synthetic-openrouter-key");
  await user.selectOptions(provider, "soniox");
  assert.equal((screen.getByLabelText("Soniox API Key") as HTMLInputElement).value, "synthetic-soniox-key");
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.api_key, "synthetic-openai-key");
    assert.equal(newSettings.groq_api_key, "synthetic-groq-key");
    assert.equal(newSettings.soniox_api_key, "synthetic-soniox-key");
    assert.equal(newSettings.openrouter_api_key, "synthetic-openrouter-key");
    assert.equal(newSettings.prompt_optimization_enabled, true);
  });
});

test("selects Soniox v5 and saves its key without replacing other keys", async () => {
  const user = userEvent.setup();
  render(<SettingsView />);
  await user.selectOptions(await screen.findByRole("combobox", { name: "Provider" }), "soniox");
  assert.equal((screen.getByRole("combobox", { name: "Transcription model" }) as HTMLSelectElement).value, "stt-rt-v5");
  const input = screen.getByLabelText("Soniox API Key") as HTMLInputElement;
  assert.equal(input.type, "password");
  await user.type(input, "  synthetic-soniox-key  ");
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.transcription_provider, "soniox");
    assert.equal(newSettings.model, "stt-rt-v5");
    assert.equal(newSettings.soniox_api_key, "synthetic-soniox-key");
    assert.equal(newSettings.api_key, null);
    assert.equal(newSettings.groq_api_key, null);
    assert.equal(newSettings.language, "pt");
  });
});

test("keeps an existing Soniox key when saving unrelated settings", async () => {
  resetTauriMocks({ settings: makeSettings({
    transcription_provider: "soniox", model: "stt-rt-v5",
    soniox_api_key_present: true, soniox_api_key_masked: "son...test",
  }) });
  const user = userEvent.setup();
  render(<SettingsView />);
  const input = await screen.findByLabelText("Soniox API Key") as HTMLInputElement;
  assert.equal(input.value, "");
  assert.equal(input.placeholder, "son...test");
  await user.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => {
    const call = tauriMocks.invoke.mock.calls.find(([command]) => command === "save_settings");
    assert.ok(call);
    const { newSettings } = call[1] as { newSettings: import("./appTypes").SaveSettingsPayload };
    assert.equal(newSettings.soniox_api_key, null);
  });
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
