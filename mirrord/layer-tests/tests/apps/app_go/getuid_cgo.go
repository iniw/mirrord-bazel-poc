//go:build cgo

package main

// #include <unistd.h>
import "C"

// getuid calls libc's `getuid`, which the layer hooks. Go's own `os.Getuid` makes the system call
// directly on Linux, so it would bypass the hook.
func getuid() int {
	return int(C.getuid())
}
