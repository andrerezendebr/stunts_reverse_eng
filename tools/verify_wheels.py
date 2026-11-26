"""Execute original wheel prediction, stopping before terrain search at 1513E."""
import argparse
import csv
import random
import struct
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_SS, UC_X86_REG_CS, UC_X86_REG_SP, UC_X86_REG_BP, UC_X86_REG_IP
from dos_machine import load_image, machine, call
from verify_drivetrain import simd_file


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("game_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    image = load_image(args.binary)
    rng = random.Random(0x574845454c)
    count = 0
    with args.output.open("x", newline="", encoding="ascii") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["car", "fps", "before", "after", "base", "predicted", "inverted_adjustment"])
        for path in sorted(args.game_directory.glob("CAR*.RES")):
            simd = simd_file(path)
            for index in range(64):
                state = bytearray(rng.randrange(256) for _ in range(208))
                for offset in [0x18, 0x1a, 0x1c, 0x3e]:
                    struct.pack_into("<H",state,offset,rng.choice([0,256,512,768,1023,rng.randrange(65536)]))
                if index%4 == 0: struct.pack_into("<H",state,0x2c,0)
                state[0xc1] = index%3
                fps = 10 if index%2 else 20
                cpu = machine(image)
                for pointer,angle in [(0x60ce,0),(0x572e,256),(0x571c,512),(0x5748,768)]:
                    call(cpu,0x36f2,0x36f80,[pointer,angle])
                cpu.mem_write(0x3b770+0x2400,bytes(state))
                cpu.mem_write(0x3b770+0x2000,simd)
                cpu.mem_write(0x3b770+0x9260,struct.pack("<H",fps))
                cpu.reg_write(UC_X86_REG_SS,0x3b77)
                cpu.reg_write(UC_X86_REG_CS,0x1471)
                cpu.reg_write(UC_X86_REG_SP,0xff00)
                cpu.reg_write(UC_X86_REG_BP,0x7777)
                cpu.mem_write(0x3b770+0xff00,struct.pack("<7H",0xff00,0xf000,0x2400,0x2000,0x2800,0x2c00,0))
                cpu.emu_start(0x14da2,0x1513e,count=100000)
                if cpu.reg_read(UC_X86_REG_CS)*16+cpu.reg_read(UC_X86_REG_IP)!=0x1513e:
                    raise AssertionError("Wheel prefix did not reach boundary")
                bp=cpu.reg_read(UC_X86_REG_BP)
                def local(offset,fmt):return struct.unpack(fmt,cpu.mem_read(0x3b770+bp+offset,struct.calcsize(fmt)))
                writer.writerow([path.name,fps,state.hex(),bytes(cpu.mem_read(0x3b770+0x2400,208)).hex(),
                                 ';'.join(map(str,local(-374,"<12i"))),';'.join(map(str,local(-448,"<12i"))),local(-240,"<h")[0]])
                count+=1
    print(f"Executed {count} original four-wheel predictions across all cars")


if __name__ == "__main__":main()
