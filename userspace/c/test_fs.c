#include "include/syscall.h"

intptr_t exit(uintptr_t status);

// This is a small C program used to test filesystem syscalls from userland
int main(void) {

    intptr_t serial = open(&"/dev/serial");
    write(serial, &"User print\n", 11);

    exit(0);
    return 0;
}
