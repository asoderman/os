default:
    @just --list

output_dir := "uefi_boot_partition"
bootloader_manifest := "../kloader/Cargo.toml"
bootloader_cargo_config := "../kloader/.cargo/config.toml"
bootloader_build_dir := "bootloader"
bootloader_src_dir := "../kloader/"
qemu_success_value := "33"

# m1 mac cannot properly run SMP cores
CPU_CORES := if os() == "macos" { "1" } else { "4" }

#macos
OVMF_DIR_macos := "/usr/local/ovmf/OVMF.fd/OVMF_CODE-pure-efi.fd"
#linux
OVMF_DIR_linux := "/usr/share/ovmf/OVMF.fd"

OVMF_DIR := if os() == "linux" { OVMF_DIR_linux } else { OVMF_DIR_macos }

#build_dir := env_var('OUT_DIR')
#bin := env_var('CARGO_BIN_NAME')
#cargo_output := "{{build_dir}}{{bin}}"
uefi_target := join(output_dir, "kernel/kernel.elf")

userspace_c :=  "userspace/c"
userspace := "userspace"
userspace_target := "target/userspace"

# cargo build
build:
    cargo build

# cargo test
test:
    cargo test

clean:
    rm -rf {{output_dir}}

bootloader: clean
    ./build_bootloader.sh {{bootloader_src_dir}}

# Copy the binary to the uefi image
update_dir OUTPUT:
    -mkdir {{parent_directory(uefi_target)}}
    cp -r "{{OUTPUT}}" "{{uefi_target}}"

update_and_run OUTPUT:
    just update_dir {{OUTPUT}}
    just qemu

graphics := "false"
graphics_flag := if graphics == "false" {
                    "-nographic"
                } else {
                    "-serial stdio"
                }
# TODO: env var?
debug := "false" 
debug_flags := if debug != "false" { "-s -S" } else { "" }

qemu *FLAGS:
    ## TODO: handle other exit statuses so tests can properly fail
    -qemu-system-x86_64 \
        -bios \
        {{OVMF_DIR}} \
        -drive \
        file=fat:rw:{{output_dir}},format=raw \
        -no-shutdown \
        --no-reboot \
        -D log.txt \
        -d int \
        -device \
        isa-debug-exit,iobase=0xf4,iosize=0x04 \
        -smp \
        {{CPU_CORES}} \
        {{graphics_flag}} \
        {{debug_flags}} \
        {{FLAGS}}

gdb:
    gdb -x debug.gdb

addr2line ADDR:
    llvm-addr2line -e {{uefi_target}} {{ADDR}}

# UNTESTED!
install_ovmf:
    #!/usr/bin/env sh
    cd ~/Downloads
    # Extract the .rpm
    # Ensure filename is correct to yours
    tar -xf edk2.git-ovmf-x64-0-20211216.193.g92ab049719.noarch.rpm
    # Next rename and move the file into a safe place
    sudo mkdir -p {{OVMF_DIR}}
    sudo cp -r usr/share/edk2.git/ovmf-x64/OVMF_CODE-pure-efi.fd {{OVMF_DIR}}

# Set the crate-type in syscall Cargo.toml. This is necessary due to bugs in cargo
modify_libsyscall_crate_type ARG:
    cd syscall && python3 crate_type.py {{ARG}}

# NOTE: Before running this build change the crate-type Cargo.toml from "lib" to
# "staticlib" then revert the change afterwards otherwise the kernel won't build!
# Build the syscall library as a static library for non rust programs. SEE NOTE in Justfile
build_libsyscall_static:
    just modify_libsyscall_crate_type staticlib
    cd syscall && cargo build \
    --features staticlib \
    --release \
    --target x86_64-unknown-none
    #--target ../x86_64-bare.json

    cd syscall && cbindgen \
    --config cbindgen.toml \
    --crate syscall \
    --output syscall.h \
    --lang c

    -mkdir -p userspace/c/include

    cp syscall/syscall.h userspace/c/include/syscall.h
    just modify_libsyscall_crate_type lib


# Compile a C program and link it against the syscall library
cc FILE:
    just build_libsyscall_static
    clang \
    -target x86_64-linux-elf \
    -c {{FILE}} \
    -o {{without_extension(FILE)}}.o \
    -ffreestanding \
    -nostartfiles \
    -nodefaultlibs

    -mkdir target/userspace

    ld.lld \
    -o target/userspace/{{file_name(without_extension(FILE))}} \
    {{without_extension(FILE)}}.o \
    -L -l syscall/target/x86_64-unknown-none/release/libsyscall.a \
    -e main \
    # -T linker.ld

    @echo "\n\nC file compiled to target/userspace/{{file_name(without_extension(FILE))}}.elf"



# Build userspace
userspace:
    for c_file in `ls {{userspace_c}} | grep "\.c"`; \
    do just cc "{{userspace_c}}/$c_file"; \
    done

