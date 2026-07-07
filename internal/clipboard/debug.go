package clipboard

var debugf func(format string, args ...any)

func SetDebugLogger(logf func(format string, args ...any)) {
	debugf = logf
}

func logDebug(format string, args ...any) {
	if debugf != nil {
		debugf(format, args...)
	}
}
