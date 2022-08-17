#include "include/syscall.h"

int main(void) {

    intptr_t serial = open(&"/dev/serial");
    write(serial, &"Success!\n", 9);

    exit(0);
    return 0;
}
