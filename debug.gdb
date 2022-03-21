target remote localhost:1234
symbol-file uefi_boot_partition/kernel/kernel.elf
break main
c
layout next
