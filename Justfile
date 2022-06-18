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

# The base build is configured by cargo so just delegate to cargo
build:
    cargo build

test:
    cargo test

clean:
    rm -rf {{output_dir}}

bootloader: clean
    ./build_bootloader.sh {{bootloader_src_dir}}

# Copy the binary to the uefi image
update_dir OUTPUT:
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

# TODO: handle other exit statuses so tests can properly fail
qemu *FLAGS:
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
