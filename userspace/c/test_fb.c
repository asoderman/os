#include "include/syscall.h"

int main(void) {

    intptr_t fb = open(&"/dev/fb");
    uintptr_t addr = 0x80000000;
    intptr_t result = mmap(addr, 0, MemoryFlags_DEFAULT.bits, fb);

    if (result == 0) {
        k_log(&"MMAP ok\n", 8);
    }

    for(int i=0; i< (1024 * 768); i++) {
        uint32_t* ptr = addr;
        ptr[i] = 0xFFFFFFFF;
    }

    exit(0);
    return 0;
}
