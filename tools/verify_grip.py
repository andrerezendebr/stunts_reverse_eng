"""Execute full update_grip with real trig/arithmetic helpers and controlled maps."""
import argparse
import csv
import random
import struct
from pathlib import Path
from dos_machine import load_image, machine, call
from verify_drivetrain import simd_file


def elements(kind):
    cells = bytearray(900)
    x, z = 3, 4
    if kind == 1: cells[z * 30 + x] = 0x34
    if kind == 2: cells[z * 30 + x], cells[(z+1) * 30 + x-1] = 0xfd, 0x35
    if kind == 3: cells[z * 30 + x], cells[(z+1) * 30 + x] = 0xfe, 0x36
    if kind == 4: cells[z * 30 + x], cells[z * 30 + x-1] = 0xff, 0x37
    return cells


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("game_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    image = load_image(args.binary)
    if image[0x7fd6:0x7fde] != bytes.fromhex("558bec83ec105756"):
        raise ValueError("Grip entry mismatch")
    generator = random.Random(0x47524950)
    count = 0
    with args.output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["car", "detailed", "track_kind", "before_hex", "after_hex"])
        for path in sorted(args.game_directory.glob("CAR*.RES")):
            simd = simd_file(path)
            for detailed in [0, 1]:
                for surface in range(6):
                    for phase in range(12):
                        state = bytearray(208)
                        state[2], state[10] = 3, 4
                        state[0xc1] = int(phase != 0)
                        state[0xc7] = phase % 2
                        state[0xc9] = phase % 3
                        state[0xcf] = 0x80
                        surfaces = [surface] * 4
                        if phase == 7: surfaces = [4, surface, 2, 1]
                        if phase == 8: surfaces = [4, 4, surface, 1]
                        if phase == 9: surfaces = [4, 4, 4, surface]
                        state[0xc2:0xc6] = bytes(surfaces)
                        for offset, value in [
                            (0x18, generator.choice([0,1,-1,7,-7,127,128,255,1023])),
                            (0x1c, generator.choice([0,4,5,-5,35,-35,-32768])),
                            (0x20, generator.choice([-240,-8,0,8,240,32767,-32768])),
                            (0x36, generator.choice([-256,0,1,256,32767])),
                            (0x3e, generator.choice([0,1,-1,100,-100,32767])),
                            (0x42, generator.choice([0,-1,15,-16,128,-32768])),
                            (0x2a, generator.choice([0,1,255,256,4096,15000,32767,32768,65535])),
                            (0x2c, generator.randrange(65536))]:
                            struct.pack_into("<H",state,offset,value & 0xffff)
                        kind = phase % 5
                        cpu = machine(image)
                        cpu.mem_write(0x3d770,simd)
                        cpu.mem_write(0x3db70,bytes(state))
                        cpu.mem_write(0x3b770 + 0x8c3c,struct.pack("<30H",*[i*30 for i in range(30)]))
                        cpu.mem_write(0x3b770 + 0x95d2,struct.pack("<HH",0,0x9000))
                        cpu.mem_write(0x90000,bytes(elements(kind)))
                        call(cpu,0x1471,0x17fd6,[0x2400,0x2000,detailed])
                        writer.writerow([path.name,detailed,kind,state.hex(),bytes(cpu.mem_read(0x3db70,208)).hex()])
                        count += 1
    print(f"Executed {count} complete grip cases, including surfaces and banked-tile markers")


if __name__ == "__main__":
    main()
