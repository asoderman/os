#include "include/syscall.h"

int main(void) {

    execv(&"/tmp/include/success", &"");

    // Unreachable
    exit(1);
    return 0;
}
