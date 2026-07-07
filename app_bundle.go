package main

import (
	"path/filepath"
	"strings"
)

const (
	macBundleRelaunchEnv = "NTOOLS_BUNDLE_REEXEC"
	macBundleIdentifier  = "com.chsoxy.ntools"
	macBundleName        = "ntools"
)

type macBundleLaunchPlan struct {
	ShouldRelaunch bool
	AppDir         string
	ExecutablePath string
	Args           []string
}

func planMacBundleLaunch(executablePath string, appName string, args []string) macBundleLaunchPlan {
	if isInsideMacAppBundle(executablePath) {
		return macBundleLaunchPlan{}
	}

	executableDir := filepath.Dir(executablePath)
	executableName := filepath.Base(executablePath)
	if executableName == "." || executableName == string(filepath.Separator) || executableName == "" {
		executableName = appName
	}

	appDir := filepath.Join(executableDir, executableName+".app")
	bundledExecutable := filepath.Join(appDir, "Contents", "MacOS", appName)
	relaunchArgs := append([]string{bundledExecutable}, args...)

	return macBundleLaunchPlan{
		ShouldRelaunch: true,
		AppDir:         appDir,
		ExecutablePath: bundledExecutable,
		Args:           relaunchArgs,
	}
}

func isInsideMacAppBundle(path string) bool {
	normalized := filepath.ToSlash(path)
	return strings.Contains(normalized, ".app/Contents/MacOS/")
}

func macBundleInfoPlist(executableName string) string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>` + executableName + `</string>
	<key>CFBundleIconFile</key>
	<string>icons</string>
	<key>CFBundleIdentifier</key>
	<string>` + macBundleIdentifier + `</string>
	<key>CFBundleName</key>
	<string>` + macBundleName + `</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>0.0.1</string>
	<key>CFBundleVersion</key>
	<string>0.0.1</string>
	<key>LSMinimumSystemVersion</key>
	<string>12.0.0</string>
	<key>NSHighResolutionCapable</key>
	<string>true</string>
</dict>
</plist>
`
}
