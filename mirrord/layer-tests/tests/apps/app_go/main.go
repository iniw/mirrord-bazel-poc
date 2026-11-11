// The Go app that the `app_go` test runs with the layer preloaded. It prints what the test checks:
// the Go version, that its router works, whether its symbol table has the runtime symbol that the
// real layer finds Go's runtime by, and the user ID from libc, which the layer's hook replaces.
package main

import (
	"debug/elf"
	"debug/macho"
	"debug/pe"
	"fmt"
	"os"
	"runtime"

	"github.com/go-chi/chi/v5"
)

// The symbol that the real layer reads Go's version from to choose its hooks, so the layer can't
// hook a binary whose symbol table was stripped.
const versionSymbol = "runtime.buildVersion.str"

func main() {
	router := chi.NewRouter()
	router.Get("/", nil)
	fmt.Println(runtime.Version(), len(router.Routes()), hasSymbol(versionSymbol), getuid())
}

// hasSymbol returns whether the executable's symbol table has `name`.
func hasSymbol(name string) bool {
	path, err := os.Executable()
	if err != nil {
		panic(err)
	}
	var names []string
	if file, err := elf.Open(path); err == nil {
		symbols, _ := file.Symbols()
		for _, symbol := range symbols {
			names = append(names, symbol.Name)
		}
	} else if file, err := macho.Open(path); err == nil {
		if file.Symtab != nil {
			for _, symbol := range file.Symtab.Syms {
				names = append(names, symbol.Name)
			}
		}
	} else if file, err := pe.Open(path); err == nil {
		for _, symbol := range file.Symbols {
			names = append(names, symbol.Name)
		}
	}
	for _, symbol := range names {
		if symbol == name {
			return true
		}
	}
	return false
}
