//go:build darwin

package clipboard

/*
#cgo CFLAGS: -mmacosx-version-min=10.13
#cgo LDFLAGS: -framework ApplicationServices -framework CoreFoundation -mmacosx-version-min=10.13

#include <ApplicationServices/ApplicationServices.h>
#include <CoreFoundation/CoreFoundation.h>

extern void goKeyDown(int64_t keycode, uint64_t flags);
extern void goCopyShortcutPressed(void);

static CGEventRef copyEventTapCallback(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *refcon) {
	if (type != kCGEventKeyDown) {
		return event;
	}

	int64_t keycode = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
	CGEventFlags flags = CGEventGetFlags(event);

	goKeyDown(keycode, (uint64_t)flags);

	if (keycode == 8 && ((flags & kCGEventFlagMaskCommand) != 0 || (flags & kCGEventFlagMaskAlternate) != 0 || (flags & kCGEventFlagMaskControl) != 0)) {
		goCopyShortcutPressed();
	}

	return event;
}

static bool requestAccessibilityTrust(void) {
	const void *keys[] = { kAXTrustedCheckOptionPrompt };
	const void *values[] = { kCFBooleanTrue };
	CFDictionaryRef options = CFDictionaryCreate(
		kCFAllocatorDefault,
		keys,
		values,
		1,
		&kCFCopyStringDictionaryKeyCallBacks,
		&kCFTypeDictionaryValueCallBacks
	);
	bool trusted = AXIsProcessTrustedWithOptions(options);
	CFRelease(options);
	return trusted;
}

static CFMachPortRef createCopyEventTap(void) {
	return CGEventTapCreate(
		kCGSessionEventTap,
		kCGHeadInsertEventTap,
		kCGEventTapOptionListenOnly,
		CGEventMaskBit(kCGEventKeyDown),
		copyEventTapCallback,
		NULL
	);
}

static void runCopyEventTap(CFMachPortRef eventTap) {
	CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0);
	CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
	CGEventTapEnable(eventTap, true);
	CFRunLoopRun();
	CFRelease(source);
	CFRelease(eventTap);
}
*/
import "C"

import (
	"errors"
	"strings"
	"sync/atomic"
	"unsafe"
)

var darwinCopyShortcutHandler atomic.Value

func StartCopyShortcutListener(onCopy func()) error {
	if onCopy == nil {
		return errors.New("copy shortcut listener requires a callback")
	}

	trusted := bool(C.requestAccessibilityTrust())
	logDebug("[cv-fun] macOS accessibility trusted: %v\n", trusted)

	eventTap := C.createCopyEventTap()
	if unsafe.Pointer(eventTap) == nil {
		return errors.New("failed to create macOS copy shortcut event tap; grant Accessibility permission to cv-fun")
	}

	darwinCopyShortcutHandler.Store(onCopy)

	go C.runCopyEventTap(eventTap)

	return nil
}

//export goKeyDown
func goKeyDown(keycode C.int64_t, flags C.uint64_t) {
	code := int64(keycode)
	logDebug("[cv-fun] key down: key=%s keycode=%d modifiers=%s flags=0x%x\n", macKeyName(code), code, formatMacModifiers(uint64(flags)), uint64(flags))
}

//export goCopyShortcutPressed
func goCopyShortcutPressed() {
	logDebug("[cv-fun] copy shortcut detected\n")
	handler, ok := darwinCopyShortcutHandler.Load().(func())
	if ok {
		go handler()
	}
}

func formatMacModifiers(flags uint64) string {
	modifiers := make([]string, 0, 4)
	if flags&(1<<20) != 0 {
		modifiers = append(modifiers, "Command")
	}
	if flags&(1<<19) != 0 {
		modifiers = append(modifiers, "Option")
	}
	if flags&(1<<18) != 0 {
		modifiers = append(modifiers, "Control")
	}
	if flags&(1<<17) != 0 {
		modifiers = append(modifiers, "Shift")
	}
	if len(modifiers) == 0 {
		return "none"
	}
	return strings.Join(modifiers, "+")
}

func macKeyName(keycode int64) string {
	names := map[int64]string{
		0:  "A",
		1:  "S",
		2:  "D",
		3:  "F",
		4:  "H",
		5:  "G",
		6:  "Z",
		7:  "X",
		8:  "C",
		9:  "V",
		11: "B",
		12: "Q",
		13: "W",
		14: "E",
		15: "R",
		16: "Y",
		17: "T",
		31: "O",
		32: "U",
		34: "I",
		35: "P",
		37: "L",
		38: "J",
		40: "K",
		45: "N",
		46: "M",
	}
	if name, ok := names[keycode]; ok {
		return name
	}
	return "Unknown"
}
