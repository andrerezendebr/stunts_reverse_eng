"""Execute the original steering routine and generate independent Rust test vectors.

Requires Unicorn. Loads and relocates the real DOS image, including the original
ABS helper called by the steering code. No helper calls are replaced with mocks.
"""
import argparse
import csv
import hashlib
import random
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_16
from unicorn.x86_const import UC_X86_REG_BP, UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_IP


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    binary = args.binary.read_bytes()
    if hashlib.sha256(binary).hexdigest() != "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b":
        raise ValueError("Unsupported executable SHA256")
    header = struct.unpack_from("<H", binary, 8)[0] * 16
    load_image = bytearray(binary[header:])
    # DOS adds its load segment to each segment word listed in the MZ header.
    count = struct.unpack_from("<H", binary, 6)[0]
    relocations = struct.unpack_from("<H", binary, 24)[0]
    for index in range(count):
        offset, segment = struct.unpack_from("<HH", binary, relocations + index * 4)
        position = segment * 16 + offset
        value = struct.unpack_from("<H", load_image, position)[0]
        struct.pack_into("<H", load_image, position, (value + 0x1000) & 0xffff)
    prefix = bytes.fromhex("558bec83ec0657568b3e968ea1a28eb10ad3e824fc")
    if load_image[0x87b2:0x87b2 + len(prefix)] != prefix:
        raise ValueError("Steering instructions do not match")
    speed_cases = [0, 1, 4095, 4096, 8192, 32767, 32768, 53248, 61440, 65535]
    angle_cases = [-32768, -241, -240, -100, -8, -7, -2, -1, 0, 1, 2, 7, 8, 100, 240, 241, 32767]
    cases = [(angle, speed, command, fps)
             for fps in [10, 20]
             for speed in speed_cases
             for angle in angle_cases
             for command in range(4)]
    generator = random.Random(0x5354454552)
    cases += [(generator.randrange(-32768, 32768), generator.randrange(65536), generator.randrange(4), generator.choice([10, 20, 30])) for _ in range(128)]
    results = []
    for angle, speed, command, fps in cases:
        cpu = Uc(UC_ARCH_X86, UC_MODE_16)
        cpu.mem_map(0, 1024 * 1024)
        cpu.mem_write(0x10000, bytes(load_image))
        cpu.reg_write(UC_X86_REG_CS, 0x1471)
        cpu.reg_write(UC_X86_REG_DS, 0x3b77)
        cpu.reg_write(UC_X86_REG_SS, 0x6000)
        cpu.reg_write(UC_X86_REG_SP, 0xff00)
        cpu.reg_write(UC_X86_REG_BP, 0x7777)
        # Actual DS-relative fields read/written by the routine.
        cpu.mem_write(0x3b770 + 0x8e96, struct.pack("<h", angle))
        cpu.mem_write(0x3b770 + 0x8ea2, struct.pack("<H", speed))
        cpu.mem_write(0x3b770 + 0x9260, struct.pack("<H", fps))
        cpu.mem_write(0x3b770 + 0x9278, struct.pack("<H", 0x64a if fps == 10 else 0x60a))
        # Far return to F000:FF00, followed by the promoted steering argument.
        cpu.mem_write(0x6ff00, struct.pack("<HHH", 0xff00, 0xf000, command))
        cpu.emu_start(0x187b2, 0xfff00, count=2000)
        if cpu.reg_read(UC_X86_REG_CS) != 0xf000 or cpu.reg_read(UC_X86_REG_IP) != 0xff00:
            raise AssertionError("Routine did not reach its far-return sentinel")
        if cpu.reg_read(UC_X86_REG_BP) != 0x7777 or cpu.reg_read(UC_X86_REG_SP) != 0xff04:
            raise AssertionError("Routine did not restore its stack frame")
        result = struct.unpack("<h", cpu.mem_read(0x3b770 + 0x8e96, 2))[0]
        results.append((angle, speed, command, fps, result))
    with args.output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["angle", "speed", "input", "fps", "dos_result"])
        writer.writerows(results)
    print(f"Executed original steering routine for {len(results)} vectors")


if __name__ == "__main__":
    main()
