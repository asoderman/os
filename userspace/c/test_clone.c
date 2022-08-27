#include "include/syscall.h"

int clone_main(void) {

    int test_var = 42;
    test_var += 1;

    exit(0);
    return 0;
}

// This is a small C program used to test filesystem syscalls from userland
int main(void) {

    clone(clone_main, 0);

    while (1) {}

    exit(0);
    return 0;
}
