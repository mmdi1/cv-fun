package main

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestMacBundleLaunchPlanSkipsBundledExecutable(t *testing.T) {
	plan := planMacBundleLaunch("/tmp/cv-fun.app/Contents/MacOS/cv-fun", "cv-fun", nil)

	if plan.ShouldRelaunch {
		t.Fatalf("expected bundled executable not to relaunch: %#v", plan)
	}
}

func TestMacBundleLaunchPlanWrapsBareExecutable(t *testing.T) {
	plan := planMacBundleLaunch("/tmp/bin/cv-fun", "cv-fun", []string{"--debug"})

	if !plan.ShouldRelaunch {
		t.Fatalf("expected bare executable to relaunch through app bundle")
	}
	if plan.AppDir != filepath.FromSlash("/tmp/bin/cv-fun.app") {
		t.Fatalf("unexpected app dir: %q", plan.AppDir)
	}
	if plan.ExecutablePath != filepath.FromSlash("/tmp/bin/cv-fun.app/Contents/MacOS/cv-fun") {
		t.Fatalf("unexpected executable path: %q", plan.ExecutablePath)
	}
	if len(plan.Args) != 2 || plan.Args[0] != plan.ExecutablePath || plan.Args[1] != "--debug" {
		t.Fatalf("expected relaunch args to preserve user args, got %#v", plan.Args)
	}
}

func TestMacBundleLaunchPlanUsesExecutableNameForBundle(t *testing.T) {
	plan := planMacBundleLaunch(filepath.Join("tmp", "custom-tool"), "cv-fun", nil)

	if runtime.GOOS == "windows" {
		t.Skip("relative path semantics are platform-specific here")
	}
	if plan.AppDir != filepath.Join("tmp", "custom-tool.app") {
		t.Fatalf("expected bundle name to follow executable name, got %q", plan.AppDir)
	}
}

func TestMacBundleInfoPlistUsesIconFileWithoutAssetCatalogName(t *testing.T) {
	plist := macBundleInfoPlist("cv-fun")

	if strings.Contains(plist, "CFBundleIconName") {
		t.Fatalf("expected generated plist to avoid CFBundleIconName so macOS uses icons.icns")
	}
	if !strings.Contains(plist, "<string>com.chsoxy.cv-fun</string>") {
		t.Fatalf("expected generated plist to use product bundle identifier")
	}
	if !strings.Contains(plist, "<key>CFBundleIconFile</key>") || !strings.Contains(plist, "<string>icons</string>") {
		t.Fatalf("expected generated plist to reference icons.icns")
	}
}

func TestDarwinInfoPlistUsesProductMetadata(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("build", "darwin", "Info.plist"))
	if err != nil {
		t.Fatalf("read darwin Info.plist: %v", err)
	}
	plist := string(data)

	legacyBundleID := "com." + "example." + macBundleName
	legacyProductName := "My " + "Product"
	if strings.Contains(plist, legacyBundleID) || strings.Contains(plist, legacyProductName) {
		t.Fatalf("darwin Info.plist still contains template metadata")
	}
	if strings.Contains(plist, "CFBundleIconName") {
		t.Fatalf("darwin Info.plist should avoid CFBundleIconName so macOS uses icons.icns")
	}
	if !strings.Contains(plist, "com.chsoxy.cv-fun") || !strings.Contains(plist, "cv-fun") {
		t.Fatalf("darwin Info.plist should contain product metadata")
	}
}

func TestDarwinPackageTaskUsesIcnsInsteadOfAssetCatalog(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("build", "darwin", "Taskfile.yml"))
	if err != nil {
		t.Fatalf("read darwin Taskfile.yml: %v", err)
	}
	taskfile := string(data)

	if strings.Contains(taskfile, "cp build/darwin/Assets.car") {
		t.Fatalf("darwin package task should not copy Assets.car into app bundles")
	}
	if !strings.Contains(taskfile, "rm -f \"{{.BIN_DIR}}/{{.APP_NAME}}.app/Contents/Resources/Assets.car\"") {
		t.Fatalf("darwin package task should remove stale production Assets.car")
	}
	if !strings.Contains(taskfile, "rm -f \"{{.BIN_DIR}}/{{.APP_NAME}}.dev.app/Contents/Resources/Assets.car\"") {
		t.Fatalf("darwin run task should remove stale dev Assets.car")
	}
}
