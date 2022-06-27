#include <stdint.h>

#include "include/syscall.h"

void exit(uintptr_t status);
void k_log(const uint8_t *ptr, uintptr_t len);
void sleep(uintptr_t seconds);

// This is a small C program used to test syscalls from userland
int main(void) {

    char msg[] = "hello world";
    unsigned int len = 11;

    sleep(6);
    k_log(msg, len);
    exit(0);
    return 0;
}
