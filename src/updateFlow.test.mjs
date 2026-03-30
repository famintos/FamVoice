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

  return mainViewSource.slice(effectIndex, effectIndex + 1500);
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

  return settingsViewSource.slice(loadingIndex, loadingIndex + 2000);
}

function getRefreshUpdateBlock() {
  const refreshIndex = settingsViewSource.indexOf("const refreshUpdate = async () => {");
  assert.notEqual(refreshIndex, -1, "expected refreshUpdate helper in SettingsView.tsx");

  const effectIndex = settingsViewSource.indexOf("useEffect(() => {", refreshIndex);
  assert.notEqual(effectIndex, -1, "expected refreshUpdate helper boundary in SettingsView.tsx");

  return settingsViewSource.slice(refreshIndex, effectIndex);
}

function getTernaryBranchBlock(startMarker, endMarker, branchName) {
  const startIndex = settingsViewSource.indexOf(startMarker);
  assert.notEqual(startIndex, -1, `expected ${branchName} branch in SettingsView.tsx`);

  const endIndex = settingsViewSource.indexOf(endMarker, startIndex + startMarker.length);
  assert.notEqual(endIndex, -1, `expected ${branchName} branch terminator in SettingsView.tsx`);

  return settingsViewSource.slice(startIndex, endIndex);
}

test("startup update check stores availability without auto-installing", () => {
  const updateEffectBlock = getStartupUpdateEffectBlock();

  assert.doesNotMatch(updateEffectBlock, /downloadAndInstall\(\)/);
  assert.match(updateEffectBlock, /setPendingUpdate\(update\)/);
  assert.match(updateEffectBlock, /if \(!hasDismissedUpdateNoticeRef\.current\) \{\s*setIsUpdateNoticeOpen\(true\);\s*\}/);
  assert.match(updateEffectBlock, /setIsUpdateNoticeOpen\(true\)/);
  assert.doesNotMatch(updateEffectBlock, /setPendingUpdate\(update\);\s*setIsUpdateNoticeOpen\(true\);/);
});

test("main view keeps the startup update check from reopening after dismissal", () => {
  const updateEffectBlock = getStartupUpdateEffectBlock();
  const noticeBlock = getUpdateNoticeBlock();

  assert.match(mainViewSource, /const hasDismissedUpdateNoticeRef = useRef\(false\);/);
  assert.match(mainViewSource, /const dismissUpdateNotice = \(\) => \{\s*hasDismissedUpdateNoticeRef\.current = true;\s*setIsUpdateNoticeOpen\(false\);\s*\};/);
  assert.match(updateEffectBlock, /if \(!hasDismissedUpdateNoticeRef\.current\) \{\s*setIsUpdateNoticeOpen\(true\);\s*\}/);
  assert.match(noticeBlock, /onClick=\{dismissUpdateNotice\}[\s\S]*?aria-label="Dismiss update notice"/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*dismissUpdateNotice\(\);\s*void handleOpenSettings\(\);\s*\}\}/);
  assert.match(noticeBlock, /Open settings/);
});

test("main view keeps a one-shot dismissible update notice with version text", () => {
  const noticeBlock = getUpdateNoticeBlock();
  const updatePanelWithActionPattern =
    /className="absolute inset-x-1\.5 top-1\.5 z-20 no-drag rounded-lg bg-transparent p-2"[\s\S]*?<button[\s\S]*?onClick=\{\(\) => \{\s*dismissUpdateNotice\(\);\s*void handleOpenSettings\(\);[\s\S]*?\}\}[\s\S]*?>\s*Open settings/u;

  assert.match(mainViewSource, /const \[isUpdateNoticeOpen, setIsUpdateNoticeOpen\] = useState\(false\);/);
  assert.match(mainViewSource, /const hasDismissedUpdateNoticeRef = useRef\(false\);/);
  assert.match(noticeBlock, /className="absolute inset-x-1\.5 top-1\.5 z-20 no-drag rounded-lg bg-transparent p-2"/);
  assert.match(noticeBlock, updatePanelWithActionPattern);
  assert.match(noticeBlock, /Update available/);
  assert.match(noticeBlock, /v\{pendingUpdate\.version\}/);
  assert.match(noticeBlock, /onClick=\{\(\) => \{\s*dismissUpdateNotice\(\);\s*void handleOpenSettings\(\);/);
  assert.match(noticeBlock, /onClick=\{dismissUpdateNotice\}[\s\S]*?aria-label="Dismiss update notice"/);
  assert.doesNotMatch(noticeBlock, /rounded-2xl border border-sky-400\/25 bg-\[#111827\]\/95/);
  assert.doesNotMatch(noticeBlock, /bg-sky-500\/10/);
});

test("settings view owns the manual update action and refresh logic", () => {
  const settingsUpdateSection = getSettingsUpdateSection();
  const refreshUpdateBlock = getRefreshUpdateBlock();

  assert.match(settingsViewSource, /import \{ getVersion \} from "@tauri-apps\/api\/app";/);
  assert.match(settingsViewSource, /import \{ check, type Update \} from "@tauri-apps\/plugin-updater";/);
  assert.match(settingsViewSource, /import \{ relaunch \} from "@tauri-apps\/plugin-process";/);
  assert.match(settingsViewSource, /import \{ useEffect, useRef, useState \} from "react";/);
  assert.match(settingsViewSource, /const \[availableUpdate, setAvailableUpdate\] = useState<Update \| null>\(null\);/);
  assert.match(settingsViewSource, /const \[isApplyingUpdate, setIsApplyingUpdate\] = useState\(false\);/);
  assert.match(settingsViewSource, /const \[appVersion, setAppVersion\] = useState\(""\);/);
  assert.match(settingsViewSource, /const \[updateCheckError, setUpdateCheckError\] = useState<string \| null>\(null\);/);
  assert.match(settingsViewSource, /const \[updateInstallError, setUpdateInstallError\] = useState<string \| null>\(null\);/);
  assert.match(settingsViewSource, /const updateCheckRequestIdRef = useRef\(0\);/);
  assert.match(refreshUpdateBlock, /const requestId = \+\+updateCheckRequestIdRef\.current;/);
  assert.match(refreshUpdateBlock, /if \(requestId !== updateCheckRequestIdRef\.current\) \{\s*return;\s*\}/);
  assert.match(refreshUpdateBlock, /if \(requestId === updateCheckRequestIdRef\.current\) \{\s*setIsCheckingForUpdates\(false\);\s*\}/);
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

test("settings view renders the initial loading state inline", () => {
  const settingsLoadingBlock = getSettingsLoadingBlock();

  assert.ok(settingsLoadingBlock.includes('rounded-2xl border border-white/10 bg-black/20'));
  assert.ok(settingsLoadingBlock.includes("Loading"));
  assert.ok(settingsLoadingBlock.includes("Loading settings..."));
  assert.ok(!settingsLoadingBlock.includes("bg-[#0f0f13] text-white overflow-hidden border border-white/10 rounded-xl"));
});

test("settings view surfaces update check failures instead of falling back to no-update copy", () => {
  const loadingBranch = getTernaryBranchBlock(
    "isCheckingForUpdates ? (",
    ") : updateCheckError ? (",
    "update check loading",
  );
  const errorBranch = getTernaryBranchBlock(
    "updateCheckError ? (",
    ") : availableUpdate ? (",
    "update check error",
  );

  assert.ok(loadingBranch.includes('className="py-2 text-slate-200"'));
  assert.ok(errorBranch.includes('className="py-2 text-red-400"'));
  assert.ok(errorBranch.includes("Could not check for updates."));
});

test("settings view uses inline update states for availability and install failures", () => {
  const availableBranch = getTernaryBranchBlock(
    "availableUpdate ? (",
    ") : (",
    "available update",
  );
  const installErrorIndex = settingsViewSource.indexOf("updateInstallError && (");
  assert.notEqual(installErrorIndex, -1, "expected update install error branch in SettingsView.tsx");
  const installErrorBranch = settingsViewSource.slice(installErrorIndex, installErrorIndex + 600);

  assert.ok(availableBranch.includes('className="py-2 text-slate-200"'));
  assert.ok(availableBranch.includes("Update available"));
  assert.ok(availableBranch.includes("currentVersionRow"));
  assert.ok(installErrorBranch.includes('className="py-2 text-red-400"'));
  assert.ok(installErrorBranch.includes("Update installation failed."));
  assert.ok(installErrorBranch.includes("{updateInstallError}"));
});
