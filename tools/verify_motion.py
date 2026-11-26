"""Execute rotation composition and the preparation prefix of player physics.

The prefix stops at 14ECE, before suspension/contact/position integration.
"""
import argparse
import csv
import random
import struct
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_AX,UC_X86_REG_CS,UC_X86_REG_SS,UC_X86_REG_SP,UC_X86_REG_BP,UC_X86_REG_IP
from dos_machine import load_image,machine,call


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary",type=Path);parser.add_argument("output_directory",type=Path)
    args=parser.parse_args();image=load_image(args.binary);args.output_directory.mkdir(parents=True,exist_ok=True)
    generator=random.Random(0x4d4f54494f4e)
    def initialize_rotations(cpu):
        # init_polyinfo creates these four Y matrices before physics runs. A raw
        # executable image still contains zero buffers at these addresses.
        for pointer,angle in [(0x60ce,0),(0x572e,256),(0x571c,512),(0x5748,768)]:
            call(cpu,0x36f2,0x36f80,[pointer,angle])
    edges=[0,1,256,512,768]
    cases=[(z,x,y,flags)for z in edges for x in edges for y in edges for flags in [0,1]]
    cases += [tuple(generator.randrange(65536)for _ in range(3))+(generator.randrange(4),)for _ in range(506)]
    with(args.output_directory/"rotation-zxy-dos.csv").open("x",newline="",encoding="ascii")as stream:
        writer=csv.writer(stream,lineterminator="\n");writer.writerow(["z","x","y","flags","matrix"])
        # Reuse the machine to exercise the original Y-rotation cache as well.
        cpu=machine(image);initialize_rotations(cpu)
        for z,x,y,flags in cases:
            call(cpu,0x24d6,0x261fa,[z,x,y,flags])
            pointer=cpu.reg_read(UC_X86_REG_AX)
            result=struct.unpack("<9h",cpu.mem_read(0x3b770+pointer,18))
            writer.writerow([z,x,y,flags,';'.join(map(str,result))])
    with(args.output_directory/"motion-prefix-dos.csv").open("x",newline="",encoding="ascii")as stream:
        writer=csv.writer(stream,lineterminator="\n");writer.writerow(["fps","before_hex","after_hex","speed_scaled","front_quarter"])
        for index in range(512):
            fps=10 if index%2==0 else 20
            state=bytearray(generator.randrange(256)for _ in range(208))
            for offset in [0x18,0x1a,0x1c]:struct.pack_into("<H",state,offset,generator.choice([0,1,256,512,768,1023,generator.randrange(65536)]))
            state[0xc1]=index%3
            cpu=machine(image);initialize_rotations(cpu)
            cpu.mem_write(0x3db70,bytes(state));cpu.mem_write(0x3b770+0x9260,struct.pack("<H",fps))
            # Near pointers to stack-local vectors require the runtime DGROUP
            # convention SS=DS. Earlier leaf kernels did not pass local pointers.
            cpu.reg_write(UC_X86_REG_SS,0x3b77)
            cpu.reg_write(UC_X86_REG_CS,0x1471);cpu.reg_write(UC_X86_REG_SP,0xff00);cpu.reg_write(UC_X86_REG_BP,0x7777)
            cpu.mem_write(0x3b770+0xff00,struct.pack("<7H",0xff00,0xf000,0x2400,0x2000,0x2600,0x2800,0))
            cpu.emu_start(0x14da2,0x14ece,count=20000)
            if cpu.reg_read(UC_X86_REG_CS)*16+cpu.reg_read(UC_X86_REG_IP)!=0x14ece:raise AssertionError("Motion prefix did not reach boundary")
            bp=cpu.reg_read(UC_X86_REG_BP)
            speed=struct.unpack("<H",cpu.mem_read(0x3b770+bp-230,2))[0]
            front=struct.unpack("<h",cpu.mem_read(0x3b770+bp-322,2))[0]
            writer.writerow([fps,state.hex(),bytes(cpu.mem_read(0x3db70,208)).hex(),speed,front])
    print(f"Executed {len(cases)} composed rotations and 512 player-physics preparation prefixes")


if __name__=="__main__":main()
