//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c -fblocks
#cgo LDFLAGS: -framework AppKit

#import <AppKit/AppKit.h>

static void ntoolsSetDockPolicyAccessory(void) {
	void (^applyPolicy)(void) = ^{
		[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
	};

	if ([NSThread isMainThread]) {
		applyPolicy();
	} else {
		dispatch_sync(dispatch_get_main_queue(), applyPolicy);
	}
}

static void ntoolsSetDockPolicyRegular(void) {
	void (^applyPolicy)(void) = ^{
		[NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
	};

	if ([NSThread isMainThread]) {
		applyPolicy();
	} else {
		dispatch_sync(dispatch_get_main_queue(), applyPolicy);
	}
}

static void ntoolsApplyApplicationIcon(void *icon, int length) {
	if (icon == NULL || length <= 0) {
		return;
	}

	void (^applyIcon)(void) = ^{
		NSData *data = [NSData dataWithBytes:icon length:length];
		NSImage *image = [[NSImage alloc] initWithData:data];
		if (image != nil) {
			[NSApp setApplicationIconImage:image];
		}
	};

	if ([NSThread isMainThread]) {
		applyIcon();
	} else {
		dispatch_sync(dispatch_get_main_queue(), applyIcon);
	}
}

static NSImage* ntoolsImageFromBytes(void *icon, int length) {
	if (icon == NULL || length <= 0) {
		return nil;
	}
	NSData *data = [NSData dataWithBytes:icon length:length];
	return [[NSImage alloc] initWithData:data];
}

static void ntoolsApplyWindowAnimationIcon(void *window, void *icon, int length) {
	if (window == NULL || icon == NULL || length <= 0) {
		return;
	}

	void (^applyIcon)(void) = ^{
		NSWindow *nsWindow = (NSWindow *)window;
		NSImage *image = ntoolsImageFromBytes(icon, length);
		if (image != nil) {
			[nsWindow setMiniwindowImage:image];
		}
	};

	if ([NSThread isMainThread]) {
		applyIcon();
	} else {
		dispatch_sync(dispatch_get_main_queue(), applyIcon);
	}
}

static void ntoolsDisableWindowAnimations(void *window) {
	if (window == NULL) {
		return;
	}

	void (^applyBehavior)(void) = ^{
		NSWindow *nsWindow = (NSWindow *)window;
		[nsWindow setAnimationBehavior:NSWindowAnimationBehaviorNone];
	};

	if ([NSThread isMainThread]) {
		applyBehavior();
	} else {
		dispatch_sync(dispatch_get_main_queue(), applyBehavior);
	}
}

static void ntoolsShowWindowWithoutAnimation(void *window) {
	if (window == NULL) {
		return;
	}

	void (^showWindow)(void) = ^{
		NSWindow *nsWindow = (NSWindow *)window;
		[NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
			[context setDuration:0.0];
			[context setAllowsImplicitAnimation:NO];
			[nsWindow setAnimationBehavior:NSWindowAnimationBehaviorNone];
			[nsWindow makeKeyAndOrderFront:nil];
			[nsWindow makeKeyWindow];
			[NSApp activateIgnoringOtherApps:YES];
		} completionHandler:nil];
	};

	if ([NSThread isMainThread]) {
		showWindow();
	} else {
		dispatch_sync(dispatch_get_main_queue(), showWindow);
	}
}

static void ntoolsHideWindowWithoutAnimation(void *window) {
	if (window == NULL) {
		return;
	}

	void (^hideWindow)(void) = ^{
		NSWindow *nsWindow = (NSWindow *)window;
		[NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
			[context setDuration:0.0];
			[context setAllowsImplicitAnimation:NO];
			[nsWindow setAnimationBehavior:NSWindowAnimationBehaviorNone];
			[nsWindow orderOut:nil];
		} completionHandler:nil];
	};

	if ([NSThread isMainThread]) {
		hideWindow();
	} else {
		dispatch_sync(dispatch_get_main_queue(), hideWindow);
	}
}

static void ntoolsHidePanelWithoutDockBounce(void *window) {
	if (window == NULL) {
		return;
	}

	void (^hidePanel)(void) = ^{
		NSWindow *nsWindow = (NSWindow *)window;
		[NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
			[context setDuration:0.0];
			[context setAllowsImplicitAnimation:NO];
			[nsWindow setAnimationBehavior:NSWindowAnimationBehaviorNone];
			[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
			[nsWindow orderOut:nil];
		} completionHandler:nil];
	};

	if ([NSThread isMainThread]) {
		hidePanel();
	} else {
		dispatch_sync(dispatch_get_main_queue(), hidePanel);
	}
}
*/
import "C"

import (
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func hideDockIcon() {
	application.InvokeSync(func() {
		C.ntoolsSetDockPolicyAccessory()
	})
}

func showDockIcon() {
	application.InvokeSync(func() {
		C.ntoolsSetDockPolicyRegular()
	})
}

func applyAppIcon(icon []byte) {
	if len(icon) == 0 {
		return
	}
	application.InvokeSync(func() {
		C.ntoolsApplyApplicationIcon(unsafe.Pointer(&icon[0]), C.int(len(icon)))
	})
}

func applyWindowAnimationIcon(window application.Window, icon []byte) {
	if window == nil || len(icon) == 0 {
		return
	}
	nativeWindow := window.NativeWindow()
	if nativeWindow == nil {
		return
	}
	application.InvokeSync(func() {
		C.ntoolsApplyWindowAnimationIcon(nativeWindow, unsafe.Pointer(&icon[0]), C.int(len(icon)))
	})
}

func disableWindowAnimations(window application.Window) {
	nativeWindow := nativeWindowPointer(window)
	if nativeWindow == nil {
		return
	}
	application.InvokeSync(func() {
		C.ntoolsDisableWindowAnimations(nativeWindow)
	})
}

func showWindowWithoutAnimation(window application.Window) {
	nativeWindow := nativeWindowPointer(window)
	if nativeWindow == nil {
		if window != nil {
			window.Show().Focus()
		}
		return
	}
	application.InvokeSync(func() {
		C.ntoolsShowWindowWithoutAnimation(nativeWindow)
	})
}

func hideWindowWithoutAnimation(window application.Window) {
	nativeWindow := nativeWindowPointer(window)
	if nativeWindow == nil {
		if window != nil {
			window.Hide()
		}
		return
	}
	application.InvokeSync(func() {
		C.ntoolsHideWindowWithoutAnimation(nativeWindow)
	})
}

func hidePanelWithoutDockBounce(window application.Window) {
	nativeWindow := nativeWindowPointer(window)
	if nativeWindow == nil {
		hideDockIcon()
		if window != nil {
			window.Hide()
		}
		return
	}
	application.InvokeSync(func() {
		C.ntoolsHidePanelWithoutDockBounce(nativeWindow)
	})
}

func nativeWindowPointer(window application.Window) unsafe.Pointer {
	if window == nil {
		return nil
	}
	nativeWindow := window.NativeWindow()
	if nativeWindow == nil {
		return nil
	}
	return nativeWindow
}
