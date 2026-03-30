import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function readSource(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8").replace(/\r\n/g, "\n");
}

const appCss = readSource("./App.css");
const mainView = readSource("./MainView.tsx");
const settingsView = readSource("./SettingsView.tsx");
const widgetView = readSource("./WidgetView.tsx");
const selectView = readSource("./components/Select.tsx");
const famVoiceLockup = readSource("./components/FamVoiceLockup.tsx");
const voiceWave = readSource("./components/VoiceWave.tsx");

test("App.css imports the official Faminto token stylesheet", () => {
  assert.match(appCss, /@import\s+(?:url\(\s*)?["'][^"']*brand\.css["']\s*\)?;/);
});

test("shared lockup helper composes the approved dark brand treatment", () => {
  assert.match(famVoiceLockup, /FamVoiceLogo/);
  assert.match(famVoiceLockup, /text-\[var\(--fam-text-primary\)\]/);
  assert.match(famVoiceLockup, /text-\[var\(--fam-interactive\)\]/);
});

test("main settings and widget use the shared lockup helper", () => {
  assert.match(mainView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);
  assert.match(settingsView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);
  assert.match(widgetView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);

  assert.doesNotMatch(mainView, /FamVoice<span className="text-primary">/);
  assert.doesNotMatch(settingsView, /FamVoice<span className="text-primary">/);
  assert.doesNotMatch(widgetView, /FamVoice<span className="text-primary">/);
});

test("history actions are not hover-only", () => {
  assert.doesNotMatch(mainView, /group-hover:opacity-100/);
  assert.doesNotMatch(mainView, /opacity-0[^]*group-hover:opacity-100/);
});

test("main shell keeps drag behavior on the title bar only", () => {
  assert.doesNotMatch(mainView, /<main[^>]*data-tauri-drag-region/);
  assert.match(mainView, /Header[^]*<div data-tauri-drag-region className=/);
});

test("main record and history copy uses the upgraded body scale", () => {
  assert.match(mainView, /text-xs leading-5 text-slate-400/);
  assert.match(mainView, /text-xs leading-5 text-red-100\/80/);
  assert.match(mainView, /text-xs leading-5 text-amber-50/);
  assert.match(mainView, /text-xs leading-5 text-slate-200/);
});

test("main icon-only controls expose explicit aria labels", () => {
  assert.match(mainView, /aria-label="Open settings"/);
  assert.match(mainView, /aria-label="Minimize window"/);
  assert.match(mainView, /aria-label="Close window"/);
  assert.match(mainView, /aria-label="Copy transcript"/);
  assert.match(mainView, /aria-label="Re-paste transcript"/);
  assert.match(mainView, /aria-label="Delete transcript"/);
});

test("settings icon-only controls expose explicit aria labels", () => {
  assert.match(settingsView, /aria-label="Reset hotkey to default"/);
  assert.match(settingsView, /aria-label="Delete glossary row"/);
});

test("glossary rows keep persistent labels", () => {
  assert.match(settingsView, /Spoken term/);
  assert.match(settingsView, /Replacement/);
});

test("settings helper copy and glossary content use the upgraded body scale", () => {
  assert.match(settingsView, /max-w-\[42rem\] text-sm leading-6 text-slate-500/);
  assert.match(settingsView, /text-sm leading-6 text-slate-400/);
  assert.match(settingsView, /const controlMotion = "transition-colors duration-\[var\(--fam-duration-fast\)\] ease-\[var\(--fam-ease-ease\)\]";/);
  assert.match(settingsView, /text-base text-white \$\{controlMotion\} focus-visible:border-primary/);
  assert.match(settingsView, /-&gt;<\/span>/);
});

test("select primitive is native and keeps visible focus semantics", () => {
  assert.match(selectView, /<select\b/);
  assert.doesNotMatch(selectView, /focus:outline-none/);
  assert.match(selectView, /focus-ring/);
});

test("motion classes avoid perpetual idle animation and broad transitions", () => {
  assert.doesNotMatch(appCss, /transition-all/);
  assert.doesNotMatch(appCss, /linear/);
  assert.doesNotMatch(appCss, /will-change:/);
  assert.doesNotMatch(mainView, /transition-all/);
  assert.doesNotMatch(settingsView, /transition-all/);
  assert.doesNotMatch(voiceWave, /transition-all/);
});

test("idle waveform is static and no longer uses decorative idle motion", () => {
  assert.doesNotMatch(voiceWave, /wave-idle/);
  assert.doesNotMatch(voiceWave, /pacman-dot/);
});

test("widget missing-key state does not pulse indefinitely", () => {
  assert.doesNotMatch(widgetView, /animate-pulse/);
});

test("main tabs and primary actions are no longer mono uppercase", () => {
  assert.doesNotMatch(mainView, /font-mono uppercase tracking-widest/);
});

test("history empty state explains how to create the first entry", () => {
  assert.doesNotMatch(mainView, /No history yet/);
  assert.match(mainView, /first history entry/);
});

test("settings helper copy and errors include recovery steps", () => {
  assert.doesNotMatch(settingsView, /max-w-\[42rem\] text-\[10px\] leading-4 text-slate-500/);
  assert.match(settingsView, /Retry loading settings/);
  assert.match(settingsView, /save again/);
  assert.match(settingsView, /Refresh to try again/);
  assert.match(settingsView, /try installing the update again/);
});

test("widget missing-key and error states expose a visible settings action", () => {
  assert.match(widgetView, /Open settings/);
  assert.doesNotMatch(widgetView, /title=/);
  assert.match(widgetView, /const settingsAction = \(\s*<button/);
  assert.match(widgetView, /className="focus-ring [^"]*text-xs font-medium[^"]*"/);
});

test("widget normal states expose a visible settings action", () => {
  assert.match(widgetView, /showIssue \?[^]*: \([^]*settingsAction/);
});
