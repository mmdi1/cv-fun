package main

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestMacBundleLaunchPlanSkipsBundledExecutable(t *testing.T) {
	plan := planMacBundleLaunch("/tmp/ntools.app/Contents/MacOS/ntools", "ntools", nil)

	if plan.ShouldRelaunch {
		t.Fatalf("expected bundled executable not to relaunch: %#v", plan)
	}
}

func TestMacBundleLaunchPlanWrapsBareExecutable(t *testing.T) {
	plan := planMacBundleLaunch("/tmp/bin/ntools", "ntools", []string{"--debug"})

	if !plan.ShouldRelaunch {
		t.Fatalf("expected bare executable to relaunch through app bundle")
	}
	if plan.AppDir != filepath.FromSlash("/tmp/bin/ntools.app") {
		t.Fatalf("unexpected app dir: %q", plan.AppDir)
	}
	if plan.ExecutablePath != filepath.FromSlash("/tmp/bin/ntools.app/Contents/MacOS/ntools") {
		t.Fatalf("unexpected executable path: %q", plan.ExecutablePath)
	}
	if len(plan.Args) != 2 || plan.Args[0] != plan.ExecutablePath || plan.Args[1] != "--debug" {
		t.Fatalf("expected relaunch args to preserve user args, got %#v", plan.Args)
	}
}

func TestMacBundleLaunchPlanUsesExecutableNameForBundle(t *testing.T) {
	plan := planMacBundleLaunch(filepath.Join("tmp", "custom-tool"), "ntools", nil)

	if runtime.GOOS == "windows" {
		t.Skip("relative path semantics are platform-specific here")
	}
	if plan.AppDir != filepath.Join("tmp", "custom-tool.app") {
		t.Fatalf("expected bundle name to follow executable name, got %q", plan.AppDir)
	}
}

func TestMacBundleInfoPlistUsesIconFileWithoutAssetCatalogName(t *testing.T) {
	plist := macBundleInfoPlist("ntools")

	if strings.Contains(plist, "CFBundleIconName") {
		t.Fatalf("expected generated plist to avoid CFBundleIconName so macOS uses icons.icns")
	}
	if !strings.Contains(plist, "<string>com.chsoxy.ntools</string>") {
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

	if strings.Contains(plist, "com.example.ntools") || strings.Contains(plist, "My Product") {
		t.Fatalf("darwin Info.plist still contains template metadata")
	}
	if strings.Contains(plist, "CFBundleIconName") {
		t.Fatalf("darwin Info.plist should avoid CFBundleIconName so macOS uses icons.icns")
	}
	if !strings.Contains(plist, "com.chsoxy.ntools") || !strings.Contains(plist, "ntools") {
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
