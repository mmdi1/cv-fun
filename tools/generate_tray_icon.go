//go:build ignore

package main

import (
	"image"
	"image/color"
	"image/png"
	"math"
	"os"
)

const (
	size  = 128
	scale = 4
)

func main() {
	icon := image.NewRGBA(image.Rect(0, 0, size, size))
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			var coverage float64
			for sy := 0; sy < scale; sy++ {
				for sx := 0; sx < scale; sx++ {
					fx := float64(x) + (float64(sx)+0.5)/scale
					fy := float64(y) + (float64(sy)+0.5)/scale
					coverage += sample(fx, fy)
				}
			}
			coverage /= float64(scale * scale)
			if coverage > 0 {
				icon.SetRGBA(x, y, color.RGBA{A: uint8(math.Round(255 * clamp(coverage, 0, 1)))})
			}
		}
	}

	if err := os.MkdirAll("build", 0o755); err != nil {
		panic(err)
	}
	file, err := os.Create("build/trayicon.png")
	if err != nil {
		panic(err)
	}
	defer file.Close()
	if err := png.Encode(file, icon); err != nil {
		panic(err)
	}
}

func sample(x, y float64) float64 {
	c := cGlyph(x, y)
	v := vGlyph(x, y)
	return clamp(c+v, 0, 1)
}

func cGlyph(x, y float64) float64 {
	cx, cy := 42.0, 62.0
	radius := 36.0
	stroke := 14.0
	angle := math.Atan2(y-cy, x-cx)
	if math.Abs(angle) < 0.58 {
		return 0
	}
	distance := math.Abs(math.Hypot(x-cx, y-cy) - radius)
	return smoothStroke(distance, stroke)
}

func vGlyph(x, y float64) float64 {
	left := distanceToSegment(x, y, 72, 38, 95, 108)
	right := distanceToSegment(x, y, 118, 38, 95, 108)
	return math.Max(smoothStroke(left, 14), smoothStroke(right, 14))
}

func distanceToSegment(px, py, ax, ay, bx, by float64) float64 {
	abx := bx - ax
	aby := by - ay
	apx := px - ax
	apy := py - ay
	denominator := abx*abx + aby*aby
	if denominator == 0 {
		return math.Hypot(px-ax, py-ay)
	}
	t := clamp((apx*abx+apy*aby)/denominator, 0, 1)
	x := ax + abx*t
	y := ay + aby*t
	return math.Hypot(px-x, py-y)
}

func smoothStroke(distance float64, stroke float64) float64 {
	edge := stroke / 2
	softness := 1.3
	if distance <= edge-softness {
		return 1
	}
	if distance >= edge+softness {
		return 0
	}
	return (edge + softness - distance) / (2 * softness)
}

func clamp(value, min, max float64) float64 {
	if value < min {
		return min
	}
	if value > max {
		return max
	}
	return value
}
