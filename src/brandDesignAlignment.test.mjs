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
  assert.match(famVoiceLockup, /motion\?: "none" \| "fade-in"/);
  assert.match(famVoiceLockup, /motion === "fade-in" \? "lockup-motion--fade-in" : ""/);
  assert.match(famVoiceLockup, /text-\[var\(--fam-text-primary\)\]/);
  assert.match(famVoiceLockup, /text-\[var\(--fam-interactive\)\]/);
});

test("main settings and widget use the shared lockup helper", () => {
  assert.match(mainView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);
  assert.match(settingsView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);
  assert.match(widgetView, /import \{ FamVoiceLockup \} from "\.\/components\/FamVoiceLockup";/);
  assert.match(mainView, /<FamVoiceLockup markSize=\{14\} motion="fade-in" \/>/);

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
  assert.match(mainView, /text-\[11px\] leading-tight text-slate-400/);
  assert.match(mainView, /text-\[10px\] leading-tight text-red-100\/60/);
  assert.match(mainView, /text-\[10px\] leading-tight text-amber-50/);
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
  assert.match(settingsView, /matched case-insensitively, and can catch simple spaced or joined variants/);
});

test("settings helper copy and glossary content use the upgraded body scale", () => {
  assert.match(settingsView, /max-w-\[42rem\] text-xs leading-normal text-slate-400\/80/);
  assert.match(settingsView, /text-xs leading-normal text-slate-400/);
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
  assert.doesNotMatch(appCss, /widget-mark-loader-spin/);
  assert.doesNotMatch(appCss, /widget-mark-pulse--active/);
  assert.doesNotMatch(mainView, /transition-all/);
  assert.doesNotMatch(settingsView, /transition-all/);
  assert.doesNotMatch(voiceWave, /transition-all/);
});

test("the live wave carries the mark's asymmetric rhythm", () => {
  const block = voiceWave.match(/const PROFILE_PRESETS = \{([^]*?)\} satisfies/);
  assert.ok(block, "expected PROFILE_PRESETS in VoiceWave");

  const profiles = Object.fromEntries(
    [...block[1].matchAll(/(\w+):\s*\[([^\]]+)\]/g)].map(([, name, body]) => [
      name,
      body.split(",").map((value) => Number(value.trim())),
    ]),
  );

  assert.deepEqual(Object.keys(profiles).sort(), ["default", "large", "widget"]);

  for (const [name, profile] of Object.entries(profiles)) {
    const peak = profile.indexOf(Math.max(...profile));
    const centre = (profile.length - 1) / 2;

    assert.ok(peak < centre, `${name} profile should peak left of centre like the mark`);
    assert.notDeepEqual(
      profile,
      [...profile].reverse(),
      `${name} profile should not be a symmetric equalizer hill`,
    );
  }
});

test("the widget shows the mark as its own level meter", () => {
  const markLive = readSource("./components/FamVoiceMarkLive.tsx");

  // Bars carry live level, lines carry the transcript. One glyph, both halves working.
  assert.match(markLive, /listen<number>\("mic-level"/);
  assert.match(markLive, /mark-live--recording/);
  assert.match(markLive, /mark-live--transcribing/);
  assert.doesNotMatch(markLive, /transition-all/);
});

test("resting bars stay longer than their own stroke", () => {
  const markLive = readSource("./components/FamVoiceMarkLive.tsx");

  const stroke = Number(markLive.match(/export const STROKE = (\d+);/)?.[1]);
  assert.ok(Number.isFinite(stroke), "expected STROKE in FamVoiceMarkLive");

  const bars = [...markLive.matchAll(/\{ x: \d+, halfRest: (\d+), halfMax: (\d+) \}/g)].map(
    ([, halfRest, halfMax]) => ({ halfRest: Number(halfRest), halfMax: Number(halfMax) }),
  );

  assert.ok(bars.length >= 2, "expected the mark's bars in FamVoiceMarkLive");

  for (const { halfRest, halfMax } of bars) {
    // A round-capped line shorter than its stroke renders as a dot. Silence must
    // still read as the mark, not as a row of beads.
    assert.ok(
      halfRest * 2 > stroke * 1.5,
      `resting bar length ${halfRest * 2} must clear stroke ${stroke} to stay a bar`,
    );
    assert.ok(halfMax > halfRest, "bars must grow above the mark, never shrink below it");
  }
});

test("both halves of the live mark carry the same stroke weight", () => {
  const markLive = readSource("./components/FamVoiceMarkLive.tsx");

  // `non-scaling-stroke` renders in screen pixels instead of user units, which
  // decoupled bar weight from the viewBox: at 46px the bars came out 15px wide
  // against 6.9px lines. Bars are geometry now, so both halves scale together.
  assert.doesNotMatch(markLive, /vectorEffect/);
  assert.match(markLive, /width=\{STROKE\}/);
  assert.match(markLive, /strokeWidth=\{STROKE\}/);
  assert.match(markLive, /rx=\{STROKE \/ 2\}/);
});

test("live mark motion is reachable under reduced motion", () => {
  // Level must still report when animation is off; only the easing goes away.
  assert.match(appCss, /\.mark-live-bar \{/);
  assert.match(appCss, /\.mark-live--transcribing \.mark-live-line \{/);

  const reducedMotionBlock = appCss.slice(appCss.indexOf("@media (prefers-reduced-motion: reduce)"));

  assert.match(reducedMotionBlock, /\.mark-live--transcribing \.mark-live-line/);
  assert.match(reducedMotionBlock, /\.mark-live-bar \{\s*transition: none !important;/);
});

test("wave motion radiates from the peak, not the geometric middle", () => {
  assert.match(voiceWave, /const peakIndex = profiles\.indexOf\(Math\.max\(\.\.\.profiles\)\)/);
  assert.doesNotMatch(voiceWave, /distanceFromCenter/);
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

test("widget keeps issues inline without a visible settings action", () => {
  assert.match(widgetView, /Tray menu → Settings\./);
  assert.doesNotMatch(widgetView, /title=/);
  assert.doesNotMatch(widgetView, /const settingsAction = \(/);
  assert.doesNotMatch(widgetView, /aria-label="Settings"/);
});
