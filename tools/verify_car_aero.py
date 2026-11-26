"""Run original setup_aero_trackdata for both sides of all eleven car archives.

Requires Unicorn. Resource lookup, SIMD copying, and the DOS long-multiply
helpers execute as real machine code. Output contains the resulting 64-word tables.
"""
import argparse
import csv
import hashlib
import struct
from pathlib import Path
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16
from unicorn.x86_const import UC_X86_REG_BP, UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_IP


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("game_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    binary = args.binary.read_bytes()
    if hashlib.sha256(binary).hexdigest() != "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b":
        raise ValueError("Unsupported executable SHA256")
    header = struct.unpack_from("<H", binary, 8)[0] * 16
    image = bytearray(binary[header:])
    count = struct.unpack_from("<H", binary, 6)[0]
    relocations = struct.unpack_from("<H", binary, 24)[0]
    for index in range(count):
        offset, segment = struct.unpack_from("<HH", binary, relocations + index * 4)
        position = segment * 16 + offset
        struct.pack_into("<H", image, position, (struct.unpack_from("<H", image, position)[0] + 0x1000) & 0xffff)
    prefix = bytes.fromhex("558bec83ec025756837e0a007403e98300")
    if image[0x9ca2:0x9ca2 + len(prefix)] != prefix:
        raise ValueError("Aero setup instructions do not match")
    results = []
    for path in sorted(args.game_directory.glob("CAR*.RES")):
        resource = path.read_bytes()
        chunks = struct.unpack_from("<H", resource, 4)[0]
        ids = [resource[6 + i * 4:10 + i * 4] for i in range(chunks)]
        simd_offset = 6 + chunks * 8 + struct.unpack_from("<I", resource, 6 + chunks * 4 + ids.index(b"simd") * 4)[0]
        for side in [0, 1]:
            cpu = Uc(UC_ARCH_X86, UC_MODE_16)
            cpu.mem_map(0, 1024 * 1024)
            cpu.mem_write(0x10000, bytes(image))
            cpu.mem_write(0x50000, resource)
            cpu.reg_write(UC_X86_REG_CS, 0x1471)
            cpu.reg_write(UC_X86_REG_DS, 0x3b77)
            cpu.reg_write(UC_X86_REG_SS, 0x6000)
            cpu.reg_write(UC_X86_REG_SP, 0xff00)
            cpu.reg_write(UC_X86_REG_BP, 0x7777)
            # Provide the preallocated table pointer expected from game initialization.
            pointer = 0x95ce if side == 0 else 0x95da
            cpu.mem_write(0x3b770 + pointer, struct.pack("<HH", 0, 0x8000))
            cpu.mem_write(0x6ff00, struct.pack("<HHHHH", 0xff00, 0xf000, 0, 0x5000, side))
            cpu.emu_start(0x19ca2, 0xfff00, count=200000)
            if cpu.reg_read(UC_X86_REG_CS) != 0xf000 or cpu.reg_read(UC_X86_REG_IP) != 0xff00:
                raise AssertionError("Aero setup did not return")
            if cpu.reg_read(UC_X86_REG_BP) != 0x7777 or cpu.reg_read(UC_X86_REG_SP) != 0xff04:
                raise AssertionError("Aero setup did not restore its stack frame")
            destination = 0xa6ea if side == 0 else 0x9ed0
            copied = bytes(cpu.mem_read(0x3b770 + destination, 772))
            if copied != resource[simd_offset:simd_offset + 772]:
                raise AssertionError("Original lookup/copy produced different SIMD bytes")
            for index, value in enumerate(struct.unpack("<64h", cpu.mem_read(0x80000, 128))):
                results.append((path.name, side, index, value))
    if len(results) != 1408:
        raise AssertionError("Expected eleven cars, two sides, and 64 values per table")
    with args.output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["car", "side", "index", "dos_result"])
        writer.writerows(results)
    print("Executed 22 original aero setups and checked their SIMD copies; generated 1408 table values")


if __name__ == "__main__":
    main()
