package main

import (
	"fmt"
	"runtime"

	"github.com/go-chi/chi/v5"
)

func main() {
	router := chi.NewRouter()
	router.Get("/", nil)
	fmt.Println(runtime.Version(), len(router.Routes()))
}
