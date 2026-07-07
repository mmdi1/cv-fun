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
	iconSize = 1024
	samples  = 3
)

var (
	clear      = color.RGBA{}
	bodyTop    = color.RGBA{R: 15, G: 23, B: 42, A: 255}
	bodyBottom = color.RGBA{R: 17, G: 24, B: 39, A: 255}
	keyWhite   = color.RGBA{R: 248, G: 250, B: 252, A: 255}
	keyBlue    = color.RGBA{R: 37, G: 99, B: 235, A: 255}
	keyCyan    = color.RGBA{R: 56, G: 189, B: 248, A: 255}
	ink        = color.RGBA{R: 15, G: 23, B: 42, A: 255}
)

func main() {
	icon := image.NewRGBA(image.Rect(0, 0, iconSize, iconSize))
	for y := 0; y < iconSize; y++ {
		for x := 0; x < iconSize; x++ {
			icon.SetRGBA(x, y, supersample(x, y))
		}
	}

	file, err := os.Create("build/appicon.png")
	if err != nil {
		panic(err)
	}
	defer file.Close()
	if err := png.Encode(file, icon); err != nil {
		panic(err)
	}
}

func supersample(x, y int) color.RGBA {
	var r, g, b, a float64
	for sy := 0; sy < samples; sy++ {
		for sx := 0; sx < samples; sx++ {
			c := pixel(float64(x)+(float64(sx)+0.5)/samples, float64(y)+(float64(sy)+0.5)/samples)
			r += float64(c.R)
			g += float64(c.G)
			b += float64(c.B)
			a += float64(c.A)
		}
	}
	divisor := float64(samples * samples)
	return color.RGBA{
		R: uint8(math.Round(r / divisor)),
		G: uint8(math.Round(g / divisor)),
		B: uint8(math.Round(b / divisor)),
		A: uint8(math.Round(a / divisor)),
	}
}

func pixel(x, y float64) color.RGBA {
	if !inRoundedRect(x, y, 80, 80, 944, 944, 214) {
		return clear
	}

	c := blend(bodyTop, bodyBottom, (y-80)/864)

	if inRoundedRect(x, y, 214, 242, 600, 628, 96) {
		c = keyWhite
	}
	if cGlyph(x, y) {
		c = ink
	}
	if inRoundedRect(x, y, 306, 602, 538, 630, 14) {
		c = color.RGBA{R: 203, G: 213, B: 225, A: 255}
	}

	if inRoundedRect(x, y, 444, 398, 830, 784, 96) {
		c = blend(keyCyan, keyBlue, (y-398)/386)
	}
	if vGlyph(x, y) {
		c = keyWhite
	}
	if inRoundedRect(x, y, 534, 758, 744, 786, 14) {
		c = color.RGBA{R: 219, G: 234, B: 254, A: 255}
	}

	return c
}

func inRoundedRect(x, y, left, top, right, bottom, radius float64) bool {
	if x < left || x > right || y < top || y > bottom {
		return false
	}

	cx := clamp(x, left+radius, right-radius)
	cy := clamp(y, top+radius, bottom-radius)
	return math.Hypot(x-cx, y-cy) <= radius
}

func cGlyph(x, y float64) bool {
	cx, cy := 407.0, 453.0
	radius := 112.0
	stroke := 64.0
	angle := math.Atan2(y-cy, x-cx)
	if math.Abs(angle) < 0.58 {
		return false
	}
	distance := math.Abs(math.Hypot(x-cx, y-cy) - radius)
	return distance <= stroke/2
}

func vGlyph(x, y float64) bool {
	left := distanceToSegment(x, y, 548, 516, 638, 692)
	right := distanceToSegment(x, y, 728, 516, 638, 692)
	return math.Min(left, right) <= 32
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

func blend(a, b color.RGBA, amount float64) color.RGBA {
	amount = clamp(amount, 0, 1)
	return color.RGBA{
		R: uint8(math.Round(float64(a.R)*(1-amount) + float64(b.R)*amount)),
		G: uint8(math.Round(float64(a.G)*(1-amount) + float64(b.G)*amount)),
		B: uint8(math.Round(float64(a.B)*(1-amount) + float64(b.B)*amount)),
		A: uint8(math.Round(float64(a.A)*(1-amount) + float64(b.A)*amount)),
	}
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
