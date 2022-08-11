#include <stdint.h>

#include "include/syscall.h"

intptr_t exit(uintptr_t status);
intptr_t k_log(const uint8_t *ptr, uintptr_t len);
intptr_t sleep(uintptr_t seconds);

intptr_t munmap(const uint8_t *ptr, uintptr_t pages);
intptr_t mprotect(const uint8_t *ptr, uintptr_t pages, uintptr_t prot);

// Copy a character array to a destination. DOES NOT NULL TERMINATE
void string_copy(char* src, char* dst, uint64_t len) {
    for(uint64_t i = 0; i < len; i++) {
        dst[i] = src[i];
    }
}

// Print if the syscall result was an error
void print_status(intptr_t result) {
    if (result >= 0) {
        char success_msg[] = "Success!";
        k_log((uint8_t*)success_msg, 9);
    } else {
        char fail_msg[] = "failure!";
        k_log((uint8_t*)fail_msg, 8);
    }
}

// This is a small C program used to test syscalls from userland
int main(void) {

    char msg[] = "hello world";
    unsigned int len = 11;

    uint64_t pages = 4;
    uint8_t* addr = (uint8_t*) 0xFC000;

    print_status(mmap(addr, pages, MemoryFlags_DEFAULT.bits, 0));

    string_copy(msg, (char*) addr, len);
    sleep(6);
    k_log((uint8_t*) addr, len);

    print_status((uintptr_t)mprotect(addr, pages, MemoryFlags_READ.bits));

    print_status(munmap(addr, pages));
    exit(0);
    return 0;
}
