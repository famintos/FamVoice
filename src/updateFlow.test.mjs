import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const mainViewSource = readFileSync(new URL("./MainView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");
const settingsViewSource = readFileSync(new URL("./SettingsView.tsx", import.meta.url), "utf8")
  .replace(/\r\n/g, "\n");

function getStartupUpdateEffectBlock() {
  const effectIndex = mainViewSource.indexOf("useEffect(() => {\n    check()");
  assert.notEqual(effectIndex, -1, "expected startup update effect in MainView.tsx");

  return mainViewSource.slice(effectIndex, effectIndex + 900);
}

function getUpdateNoticeBlock() {
  const noticeIndex = mainViewSource.indexOf("pendingUpdate && isUpdateNoticeOpen && (");
  assert.notEqual(noticeIndex, -1, "expected dismissible update notice block in MainView.tsx");

  return mainViewSource.slice(noticeIndex, noticeIndex + 1800);
}

function getSettingsUpdateSection() {
  const sectionIndex = settingsViewSource.indexOf('eyebrow="Update"');
  assert.notEqual(sectionIndex, -1, "expected update section in SettingsView.tsx");

  return settingsViewSource.slice(sectionIndex - 200, sectionIndex + 4200);
}

function getSettingsLoadingBlock() {
  const loadingIndex = settingsViewSource.indexOf("if (!settings) {");
  assert.notEqual(loadingIndex, -1, "expected settings loading branch in SettingsView.tsx");

  return settingsViewSource.slice(loadingIndex, loadingIndex + 1200);
}

test("startup update check stores availability without auto-installing", () => {
  const updateEffectBlock = getStartupUpdateEffectBlock();

  assert.doesNotMatch(updateEffectBlock, /downloadAndInstall\(\)/);
  assert.match(updateEffectBlock, /setPendingUpdate\(update\)/);
  assert.match(updateEffectBlock, /if \(!hasDismissedUpdateNotice\) \{\s*setIsUpdateNoticeOpen\(true\);\s*\}/);
  assert.match(updateEffectBlock, /setIsUpdateNoticeOpen\(true\)/);
  assert.doesNotMatch(updateEffectBlock, /setPendingUpdate\(update\);\s*setIsUpdateNoticeOpen\(true\);/);
});

test("main view keeps a one-shot dismissible update notice with version text", () => {
  const noticeBlock = getUpdateNoticeBlock();
  const updatePanelWithActionPattern =
    /className="status-panel status-panel--update"[\s\S]*?<button[\s\S]*?onClick=\{\(\) => \{\s*void handleOpenSettings\(\);\s*setIsUpdateNoticeOpen\(false\);[\s\S]*?\}\}[\s\S]*?>\s*Open Settings\s*<\/button>/;

  assert.match(mainViewSource, /const \[isUpdateNoticeOpen, setIsUpdateNoticeOpen\] = useState\(false\);/);
  assert.match(mainViewSource, /const \[hasDismissedUpdateNotice, setHasDismissedUpdateNotice\] = useState\(false\);/);
  assert.match(noticeBlock, /className="status-panel status-panel--update"/);
  assert.match(noticeBlock, updatePanelWithActionPattern);
  assert.match(noticeBlock, /A new update is available/);
  assert.match(noticeBlock, /v\{pendingUpdate\.version\}/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*void handleOpenSettings\(\);\s*setIsUpdateNoticeOpen\(false\);/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*setHasDismissedUpdateNotice\(true\);\s*setIsUpdateNoticeOpen\(false\);/);
  assert.doesNotMatch(noticeBlock, /rounded-2xl border border-sky-400\/25 bg-\[#111827\]\/95/);
  assert.doesNotMatch(noticeBlock, /bg-sky-500\/10/);
});

test("settings view owns the manual update action and refresh logic", () => {
  const settingsUpdateSection = getSettingsUpdateSection();

  assert.match(settingsViewSource, /import \{ getVersion \} from "@tauri-apps\/api\/app";/);
  assert.match(settingsViewSource, /import \{ check, type Update \} from "@tauri-apps\/plugin-updater";/);
  assert.match(settingsViewSource, /import \{ relaunch \} from "@tauri-apps\/plugin-process";/);
  assert.match(settingsViewSource, /const \[availableUpdate, setAvailableUpdate\] = useState<Update \| null>\(null\);/);
  assert.match(settingsViewSource, /const \[isApplyingUpdate, setIsApplyingUpdate\] = useState\(false\);/);
  assert.match(settingsViewSource, /const \[appVersion, setAppVersion\] = useState\(""\);/);
  assert.match(settingsViewSource, /const \[updateCheckError, setUpdateCheckError\] = useState<string \| null>\(null\);/);
  assert.match(settingsViewSource, /const \[updateInstallError, setUpdateInstallError\] = useState<string \| null>\(null\);/);
  assert.match(settingsViewSource, /const currentVersionRow = \(/);
  assert.match(settingsViewSource, /await availableUpdate\.downloadAndInstall\(\);/);
  assert.match(settingsViewSource, /await relaunch\(\);/);
  assert.match(settingsViewSource, /appWindow\.onFocusChanged\(\(\{ payload: focused \}\) => \{/);
  assert.match(settingsViewSource, /if \(focused\) \{\s*void refreshUpdate\(\);\s*\}/);
  assert.match(settingsViewSource, /console\.error\("Update check failed:", error\);/);
  assert.match(settingsViewSource, /setAvailableUpdate\(null\);/);
  assert.match(settingsViewSource, /setUpdateCheckError\(String\(error\)\);/);
  assert.match(settingsViewSource, /setUpdateInstallError\(String\(error\)\);/);
  assert.ok(settingsUpdateSection.includes("currentVersionRow"));
  assert.ok(settingsUpdateSection.includes("Update available"));
  assert.ok(settingsUpdateSection.includes('isApplyingUpdate ? "Updating..." : "Update"'));
  assert.ok(settingsUpdateSection.includes("Updating..."));
});

test("settings view renders the initial loading state as a neutral status panel", () => {
  const settingsLoadingBlock = getSettingsLoadingBlock();

  assert.ok(settingsLoadingBlock.includes('className="status-panel status-panel--neutral'));
  assert.ok(settingsLoadingBlock.includes("Loading settings..."));
  assert.ok(!settingsLoadingBlock.includes("bg-[#0f0f13] text-white overflow-hidden border border-white/10 rounded-xl"));
});

test("settings view surfaces update check failures instead of falling back to no-update copy", () => {
  const settingsUpdateSection = getSettingsUpdateSection();

  assert.ok(settingsUpdateSection.includes("isCheckingForUpdates ? ("));
  assert.ok(settingsUpdateSection.includes("updateCheckError ? ("));
  assert.ok(settingsUpdateSection.includes("Unable to check for updates right now."));
  assert.ok(settingsUpdateSection.includes('className="status-panel status-panel--neutral'));
  assert.ok(settingsUpdateSection.includes('className="status-panel status-panel--error'));
});

test("settings view uses neutral status panels for available updates and error panels for install failures", () => {
  const settingsUpdateSection = getSettingsUpdateSection();

  assert.ok(settingsUpdateSection.includes("availableUpdate ? ("));
  assert.ok(settingsUpdateSection.includes("updateInstallError && ("));
  assert.ok(settingsUpdateSection.includes('className="status-panel status-panel--neutral'));
  assert.ok(settingsUpdateSection.includes('className="status-panel status-panel--error'));
  assert.ok(settingsUpdateSection.includes("Update installation failed."));
  assert.ok(settingsUpdateSection.includes("{updateInstallError}"));
});
