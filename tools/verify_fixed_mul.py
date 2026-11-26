"""Generate test vectors by executing the original DOS routine with Unicorn.

This optional analysis tool needs the `unicorn` Python package. The Rust game
tools have no Python dependency. Output is created exclusively, never replaced.
"""
import argparse
import csv
import hashlib
import random
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_16
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BP, UC_X86_REG_CS, UC_X86_REG_SP, UC_X86_REG_SS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    image = args.binary.read_bytes()
    expected_hash = "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b"
    if hashlib.sha256(image).hexdigest() != expected_hash:
        raise ValueError("Unsupported executable SHA256")
    header = struct.unpack_from("<H", image, 8)[0] * 16
    # Ghidra 2EA2:1624, with the DOS image loaded at segment 1000.
    offset = header + 0x2EA2 * 16 + 0x1624 - 0x10000
    code = bytes.fromhex("558bec8b4606f76e08d1e0d1d2d1e0d1d2d1e083d2008bc25dcb")
    if image[offset:offset + len(code)] != code:
        raise ValueError("Original instruction sequence does not match")
    edge = [-32768, -32767, -16384, -8193, -8192, -1, 0, 1, 8192, 16384, 32767]
    cases = [(a, b) for a in edge for b in edge]
    generator = random.Random(0x5354554E5453)
    cases += [(generator.randrange(-32768, 32768), generator.randrange(-32768, 32768)) for _ in range(128)]
    results = []
    for a, b in cases:
        cpu = Uc(UC_ARCH_X86, UC_MODE_16)
        cpu.mem_map(0, 1024 * 1024)
        cpu.mem_write(0x1000, code)
        cpu.reg_write(UC_X86_REG_CS, 0)
        cpu.reg_write(UC_X86_REG_SS, 0x2000)
        cpu.reg_write(UC_X86_REG_SP, 0x100)
        cpu.reg_write(UC_X86_REG_BP, 0x7777)
        # Model the far-call stack: return IP, return CS, then the two arguments.
        cpu.mem_write(0x20100, struct.pack("<HHhh", 0, 0, a, b))
        # Stop just before RETF; all arithmetic and register restoration ran.
        cpu.emu_start(0x1000, 0x1000 + len(code) - 1, count=64)
        result = cpu.reg_read(UC_X86_REG_AX)
        result = result if result < 0x8000 else result - 0x10000
        if cpu.reg_read(UC_X86_REG_BP) != 0x7777 or cpu.reg_read(UC_X86_REG_SP) != 0x100:
            raise AssertionError("Routine did not restore its stack frame")
        results.append((a, b, result))
    with args.output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["a", "b", "dos_result"])
        writer.writerows(results)
    print(f"Executed original DOS instructions for {len(results)} vectors")


if __name__ == "__main__":
    main()
