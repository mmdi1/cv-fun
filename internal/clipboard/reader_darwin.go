//go:build darwin

package clipboard

/*
#cgo CFLAGS: -mmacosx-version-min=10.13 -x objective-c
#cgo LDFLAGS: -framework Cocoa -mmacosx-version-min=10.13

#import <Cocoa/Cocoa.h>

const char* cvfunClipboardText() {
	NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
	NSString *text = [pasteboard stringForType:NSPasteboardTypeString];
	return [text UTF8String];
}
*/
import "C"

func ReadText() (string, bool) {
	text := C.cvfunClipboardText()
	if text == nil {
		return "", false
	}

	value := C.GoString(text)
	return value, value != ""
}
