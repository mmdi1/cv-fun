//go:build !darwin && !windows

package clipboard

func ReadText() (string, bool) {
	return "", false
}
