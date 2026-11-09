package main

import "C"
import "fmt"

//export Serve
func Serve() { fmt.Println("served from cgo") }

func main() {}
