#include <dlfcn.h>
#include <cstdio>

int main(int, char** argv) {
  void* h = dlopen(argv[1], RTLD_NOW);
  if (!h) {
    std::printf("dlopen failed: %s\n", dlerror());
    return 1;
  }

  reinterpret_cast<void (*)()>(dlsym(h, "Serve"))();

  return 0;
}
