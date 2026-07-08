//go:build windows

package clipboard

import (
	"errors"
	"fmt"
	"log"
	"runtime"
	"sync/atomic"
	"syscall"
	"unsafe"
)

const (
	whKeyboardLL = 13
	wmKeyDown    = 0x0100
	wmSysKeyDown = 0x0104
	vkC          = 0x43
	vkControl    = 0x11
)

var (
	user32                    = syscall.NewLazyDLL("user32.dll")
	procSetWindowsHookExW     = user32.NewProc("SetWindowsHookExW")
	procCallNextHookEx        = user32.NewProc("CallNextHookEx")
	procGetMessageW           = user32.NewProc("GetMessageW")
	procTranslateMessage      = user32.NewProc("TranslateMessage")
	procDispatchMessageW      = user32.NewProc("DispatchMessageW")
	procGetAsyncKeyState      = user32.NewProc("GetAsyncKeyState")
	windowsCopyShortcutHook   uintptr
	windowsKeyboardCallback   uintptr
	windowsCopyShortcutHandle atomic.Value
)

type keyboardHookEvent struct {
	VKCode      uint32
	ScanCode    uint32
	Flags       uint32
	Time        uint32
	DwExtraInfo uintptr
}

type windowsMessage struct {
	HWND    uintptr
	Message uint32
	WParam  uintptr
	LParam  uintptr
	Time    uint32
	Point   struct {
		X int32
		Y int32
	}
}

func StartCopyShortcutListener(onCopy func()) error {
	if onCopy == nil {
		return errors.New("copy shortcut listener requires a callback")
	}

	windowsCopyShortcutHandle.Store(onCopy)
	windowsKeyboardCallback = syscall.NewCallback(windowsKeyboardProc)

	ready := make(chan error, 1)
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()

		hook, _, err := procSetWindowsHookExW.Call(
			uintptr(whKeyboardLL),
			windowsKeyboardCallback,
			0,
			0,
		)
		if hook == 0 {
			ready <- fmt.Errorf("failed to install Windows copy shortcut hook: %w", err)
			return
		}

		windowsCopyShortcutHook = hook
		ready <- nil
		windowsMessageLoop()
	}()

	return <-ready
}

func windowsKeyboardProc(nCode int, wParam uintptr, lParam uintptr) uintptr {
	if nCode >= 0 && (wParam == wmKeyDown || wParam == wmSysKeyDown) {
		event := (*keyboardHookEvent)(unsafe.Pointer(lParam))
		if event.VKCode == vkC && isControlPressed() {
			logDebug("[cv-fun] copy shortcut detected\n")
			handler, ok := windowsCopyShortcutHandle.Load().(func())
			if ok {
				go handler()
			}
		}
	}

	next, _, _ := procCallNextHookEx.Call(windowsCopyShortcutHook, uintptr(nCode), wParam, lParam)
	return next
}

func isControlPressed() bool {
	state, _, _ := procGetAsyncKeyState.Call(uintptr(vkControl))
	return state&0x8000 != 0
}

func windowsMessageLoop() {
	var msg windowsMessage
	for {
		result, _, err := procGetMessageW.Call(uintptr(unsafe.Pointer(&msg)), 0, 0, 0)
		switch int32(result) {
		case -1:
			log.Printf("copy shortcut listener message loop failed: %v", err)
			return
		case 0:
			return
		default:
			procTranslateMessage.Call(uintptr(unsafe.Pointer(&msg)))
			procDispatchMessageW.Call(uintptr(unsafe.Pointer(&msg)))
		}
	}
}
