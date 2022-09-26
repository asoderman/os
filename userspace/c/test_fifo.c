#include "include/syscall.h"

void fifo_fn(void) {
    intptr_t fifo = open(&"/tmp/test_fifo");

    write(fifo, &"OK\n", 3);

    exit(0);
}

int main(void) {
    char fifo_path[] = "/tmp/test_fifo";

    intptr_t serial = open("/dev/serial");

    intptr_t fifo = mkfifo(fifo_path);

    clone(fifo_fn, 0);

    char read_result [4] = {0, 0, 0, 0};

    sleep(3);

    if (read(fifo, read_result, 3) != 3) {
        write(serial, &"BAD\n", 4);
        exit(1);
    }

    write(serial, read_result, 3);

    exit(0);
    return 0;
}
