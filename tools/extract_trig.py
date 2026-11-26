"""Extract the exact DOS quarter-wave table and execute both original trig helpers."""
import argparse
import csv
import random
import struct
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_AX
from dos_machine import load_image, machine, call


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("rust_output", type=Path)
    parser.add_argument("csv_output", type=Path)
    args = parser.parse_args()
    image = load_image(args.binary)
    values = struct.unpack_from("<257h", image, 0x2b770 + 0x4c84)
    if values[:4] != (0, 101, 201, 302) or values[-1] != 16384:
        raise ValueError("Quarter-wave table mismatch")
    with args.rust_output.open("x", encoding="ascii") as stream:
        stream.write("// Extracted from the verified MCGA image, DS:4C84. Do not regenerate with floating point.\n")
        stream.write("pub const SINE: [i16; 257] = [\n")
        for index in range(0, len(values), 16):
            stream.write("    " + ", ".join(map(str, values[index:index + 16])) + ",\n")
        stream.write("];\n")
    generator = random.Random(0x54524947)
    angles = list(range(1024)) + [generator.randrange(65536) for _ in range(128)]
    cpu = machine(image)
    with args.csv_output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["angle", "sin", "cos"])
        for angle in angles:
            results = []
            for entry in [0x326de, 0x3272c]:
                call(cpu, 0x2ea2, entry, [angle])
                value = cpu.reg_read(UC_X86_REG_AX)
                results.append(value if value < 32768 else value - 65536)
            writer.writerow([angle, *results])
    print(f"Extracted 257 exact table words; executed sin/cos for {len(angles)} angles")


if __name__ == "__main__":
    main()
