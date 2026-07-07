package main

import (
	"bufio"
	"fmt"
	"os"
)

func runCopyDebugMode() bool {
	if os.Getenv("NTOOLS_COPY_DEBUG") != "1" {
		return false
	}

	fmt.Println("[ntools] copy debug mode enabled")
	fmt.Println("[ntools] press keys now; press Enter here to exit")

	done := make(chan struct{})
	if err := startCopyShortcutListener(func() {
		fmt.Println("[ntools] debug copy callback fired")
	}); err != nil {
		fmt.Printf("[ntools] debug listener failed: %v\n", err)
		return true
	}

	go func() {
		_, _ = bufio.NewReader(os.Stdin).ReadString('\n')
		close(done)
	}()

	<-done
	fmt.Println("[ntools] copy debug mode stopped")
	return true
}
