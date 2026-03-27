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
  const sectionIndex = settingsViewSource.indexOf('tracking-wider">Update</h3>');
  assert.notEqual(sectionIndex, -1, "expected update section in SettingsView.tsx");

  return settingsViewSource.slice(sectionIndex - 400, sectionIndex + 2800);
}

test("startup update check stores availability without auto-installing", () => {
  const updateEffectBlock = getStartupUpdateEffectBlock();

  assert.doesNotMatch(updateEffectBlock, /downloadAndInstall\(\)/);
  assert.match(updateEffectBlock, /setPendingUpdate\(update\)/);
  assert.match(updateEffectBlock, /setIsUpdateNoticeOpen\(true\)/);
});

test("main view keeps a one-shot dismissible update notice with version text", () => {
  const noticeBlock = getUpdateNoticeBlock();

  assert.match(mainViewSource, /const \[isUpdateNoticeOpen, setIsUpdateNoticeOpen\] = useState\(false\);/);
  assert.match(mainViewSource, /const \[hasDismissedUpdateNotice, setHasDismissedUpdateNotice\] = useState\(false\);/);
  assert.match(noticeBlock, /A new update is available/);
  assert.match(noticeBlock, /v\{pendingUpdate\.version\}/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*void handleOpenSettings\(\);\s*setIsUpdateNoticeOpen\(false\);/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*setHasDismissedUpdateNotice\(true\);\s*setIsUpdateNoticeOpen\(false\);/);
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
  assert.match(settingsViewSource, /await availableUpdate\.downloadAndInstall\(\);/);
  assert.match(settingsViewSource, /await relaunch\(\);/);
  assert.match(settingsViewSource, /appWindow\.onFocusChanged\(\(\{ payload: focused \}\) => \{/);
  assert.match(settingsViewSource, /if \(focused\) \{\s*void refreshUpdate\(\);\s*\}/);
  assert.match(settingsViewSource, /console\.error\("Update check failed:", error\);/);
  assert.match(settingsViewSource, /setAvailableUpdate\(null\);/);
  assert.match(settingsViewSource, /setUpdateCheckError\(String\(error\)\);/);
  assert.match(settingsViewSource, /setUpdateInstallError\(String\(error\)\);/);
  assert.match(settingsUpdateSection, /Current version/);
  assert.match(settingsUpdateSection, /Update available/);
  assert.match(settingsUpdateSection, />\s*Update\s*</);
  assert.match(settingsUpdateSection, /Updating\.\.\./);
});

test("settings view surfaces update check failures instead of falling back to no-update copy", () => {
  const settingsUpdateSection = getSettingsUpdateSection();

  assert.match(settingsUpdateSection, /updateCheckError \? \(/);
  assert.match(settingsUpdateSection, /Unable to check for updates right now\./);
  assert.match(
    settingsUpdateSection,
    /isCheckingForUpdates \? \([\s\S]*\) : updateCheckError \? \([\s\S]*\) : availableUpdate \? \([\s\S]*\) : \([\s\S]*No update available\./,
  );
});

test("settings view preserves the available update card when installation fails", () => {
  const settingsUpdateSection = getSettingsUpdateSection();

  assert.match(settingsUpdateSection, /availableUpdate \? \(/);
  assert.match(settingsUpdateSection, /updateInstallError && \(/);
  assert.match(settingsUpdateSection, /\{updateInstallError\}/);
});
