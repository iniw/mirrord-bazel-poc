//go:build !cgo

package main

// getuid returns -1 where Go builds without cgo, like on Windows, which has no `getuid`.
func getuid() int {
	return -1
}
