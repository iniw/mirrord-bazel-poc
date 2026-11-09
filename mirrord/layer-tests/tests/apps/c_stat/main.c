#include <stdio.h>
#include <sys/stat.h>

int main(void) {
  struct stat st;
  printf("stat=%d\n", stat("/", &st));

  return 0;
}
