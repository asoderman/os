#!/bin/bash

pushd "$1"

cargo build
cargo make fat

popd

cp -r "$1/uefi_boot_partition" "./uefi_boot_partition"
echo "Copied: $1/uefi_boot_partition"

