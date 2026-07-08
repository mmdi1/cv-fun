package main

import (
	"bufio"
	"fmt"
	"os"
)

func runCopyDebugMode() bool {
	if os.Getenv("CV_FUN_COPY_DEBUG") != "1" {
		return false
	}

	fmt.Println("[cv-fun] copy debug mode enabled")
	fmt.Println("[cv-fun] press keys now; press Enter here to exit")

	done := make(chan struct{})
	if err := startCopyShortcutListener(func() {
		fmt.Println("[cv-fun] debug copy callback fired")
	}); err != nil {
		fmt.Printf("[cv-fun] debug listener failed: %v\n", err)
		return true
	}

	go func() {
		_, _ = bufio.NewReader(os.Stdin).ReadString('\n')
		close(done)
	}()

	<-done
	fmt.Println("[cv-fun] copy debug mode stopped")
	return true
}
