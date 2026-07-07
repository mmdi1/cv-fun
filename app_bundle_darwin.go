//go:build darwin

package main

import (
	_ "embed"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"
)

//go:embed build/darwin/icons.icns
var macBundleIcon []byte

func ensureMacAppBundleLaunch() error {
	if os.Getenv(macBundleRelaunchEnv) == "1" {
		return nil
	}

	executablePath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("resolve executable path: %w", err)
	}

	plan := planMacBundleLaunch(executablePath, macBundleName, os.Args[1:])
	if !plan.ShouldRelaunch {
		return nil
	}

	if err := writeMacAppBundle(plan, executablePath); err != nil {
		return err
	}

	env := append(os.Environ(), macBundleRelaunchEnv+"=1")
	return syscall.Exec(plan.ExecutablePath, plan.Args, env)
}

func writeMacAppBundle(plan macBundleLaunchPlan, sourceExecutable string) error {
	if err := os.MkdirAll(filepath.Join(plan.AppDir, "Contents", "MacOS"), 0o755); err != nil {
		return fmt.Errorf("create app executable directory: %w", err)
	}
	if err := os.MkdirAll(filepath.Join(plan.AppDir, "Contents", "Resources"), 0o755); err != nil {
		return fmt.Errorf("create app resources directory: %w", err)
	}

	if err := copyFile(sourceExecutable, plan.ExecutablePath, 0o755); err != nil {
		return fmt.Errorf("copy executable into app bundle: %w", err)
	}
	if err := os.WriteFile(filepath.Join(plan.AppDir, "Contents", "Info.plist"), []byte(macBundleInfoPlist(macBundleName)), 0o644); err != nil {
		return fmt.Errorf("write app Info.plist: %w", err)
	}
	if err := os.WriteFile(filepath.Join(plan.AppDir, "Contents", "Resources", "icons.icns"), macBundleIcon, 0o644); err != nil {
		return fmt.Errorf("write app icon: %w", err)
	}

	return nil
}

func copyFile(src string, dst string, perm os.FileMode) error {
	source, err := os.Open(src)
	if err != nil {
		return err
	}
	defer source.Close()

	temp := dst + ".tmp"
	target, err := os.OpenFile(temp, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, perm)
	if err != nil {
		return err
	}

	_, copyErr := io.Copy(target, source)
	closeErr := target.Close()
	if copyErr != nil {
		_ = os.Remove(temp)
		return copyErr
	}
	if closeErr != nil {
		_ = os.Remove(temp)
		return closeErr
	}

	if err := os.Chmod(temp, perm); err != nil {
		_ = os.Remove(temp)
		return err
	}
	if err := os.Rename(temp, dst); err != nil {
		_ = os.Remove(temp)
		return err
	}

	return nil
}
