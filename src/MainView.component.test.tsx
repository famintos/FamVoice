import assert from "node:assert/strict";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, test } from "vitest";
import { MainView } from "./MainView";
import {
  emitStatus,
  emitTauriEvent,
  makeSettings,
  resetTauriMocks,
  tauriMocks,
} from "./test/tauriMocks";

const history = [{ id: 7, text: "Olá, mundo 🌍", timestamp: 1_700_000_000_000, pinned: false }];

beforeEach(() => {
  resetTauriMocks({ history });
});

test("executes history actions against the Tauri command boundary", async () => {
  const user = userEvent.setup();
  render(<MainView />);

  await user.click(await screen.findByRole("tab", { name: "History" }));
  assert.ok(screen.getByText("Olá, mundo 🌍"));
  await user.click(screen.getByRole("button", { name: "Delete transcript" }));

  assert.ok(tauriMocks.invoke.mock.calls.some(([command, args]) =>
    command === "delete_history_item" && (args as { id: number }).id === 7));
});

test("clear-history dialog owns focus, traps Tab, closes with Escape and restores focus", async () => {
  render(<MainView />);

  fireEvent.click(await screen.findByRole("tab", { name: "History" }));
  const trigger = screen.getByRole("button", { name: "Clear history" });
  fireEvent.click(trigger);

  const dialog = document.querySelector<HTMLElement>("[role='dialog']");
  assert.ok(dialog);
  const buttons = dialog.querySelectorAll<HTMLButtonElement>("button");
  const cancel = buttons[0];
  const confirm = buttons[1];
  assert.equal(cancel.textContent?.trim(), "Cancel");
  assert.equal(confirm.textContent?.trim(), "Clear history");
  await waitFor(() => {
    if (document.activeElement !== cancel) {
      throw new Error("Cancel button has not received initial focus yet");
    }
  });

  confirm.focus();
  fireEvent.keyDown(document, { key: "Tab" });
  assert.equal(document.activeElement, cancel);

  fireEvent.keyDown(document, { key: "Escape" });
  assert.equal(document.querySelector("[role='dialog']"), null);
  assert.equal(document.activeElement, trigger);
});

test("announces changing dictation status once and keeps the delivery result visible", async () => {
  render(<MainView />);
  await screen.findByRole("tab", { name: "Record" });

  emitTauriEvent("transcript", "Texto final");
  emitStatus("success");

  await waitFor(() => {
    const liveRegions = screen.getAllByRole("status");
    assert.equal(liveRegions.length, 1);
    assert.match(liveRegions[0].textContent ?? "", /Transcript ready/i);
    assert.match(liveRegions[0].textContent ?? "", /Pasted to your app/i);
  });
});

test("renders widget error state from live Tauri events", async () => {
  resetTauriMocks({ settings: makeSettings({ widget_mode: true }) });
  render(<MainView />);

  emitTauriEvent("transcript", "No voice detected");
  emitStatus("error");

  assert.ok(await screen.findByText("No speech found."));
  assert.ok(screen.getByText("Error"));
});

test("offers a single-use retry for retained failed audio", async () => {
  const user = userEvent.setup();
  render(<MainView />);
  await screen.findByRole("tab", { name: "Record" });

  emitTauriEvent("transcript", "Provider unavailable");
  emitTauriEvent("retry-audio-state", { available: true });
  emitStatus("error");

  await user.click(await screen.findByRole("button", { name: "Retry last dictation" }));
  assert.ok(tauriMocks.invoke.mock.calls.some(([command]) => command === "retry_last_dictation"));
});

test("searches, pins and exports history through explicit controls", async () => {
  resetTauriMocks({
    history: [
      { id: 1, text: "Primeiro texto", timestamp: 2, pinned: false },
      { id: 2, text: "Reunião Faminto", timestamp: 1, pinned: true },
    ],
  });
  const user = userEvent.setup();
  render(<MainView />);

  await user.click(await screen.findByRole("tab", { name: "History" }));
  await user.type(screen.getByRole("searchbox", { name: "Search history" }), "reunião");
  assert.ok(screen.getByText("Reunião Faminto"));
  assert.equal(screen.queryByText("Primeiro texto"), null);

  await user.click(screen.getByRole("button", { name: "Unpin transcript" }));
  await user.click(screen.getByRole("button", { name: "Export history as Markdown" }));
  assert.ok(tauriMocks.invoke.mock.calls.some(([command, args]) =>
    command === "toggle_history_pin" && (args as { id: number }).id === 2));
  assert.ok(tauriMocks.invoke.mock.calls.some(([command, args]) =>
    command === "export_history" && (args as { format: string }).format === "markdown"));
});
