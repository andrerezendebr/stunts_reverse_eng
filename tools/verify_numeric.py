"""Execute original matrix, rotation and six-byte RNG routines without mocks."""
import argparse
import csv
import random
import struct
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_AX
from dos_machine import load_image,machine,call


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary",type=Path);parser.add_argument("output_directory",type=Path)
    args=parser.parse_args();image=load_image(args.binary);args.output_directory.mkdir(parents=True,exist_ok=True)
    generator=random.Random(0x4d415448)
    def words(values):return ';'.join(map(str,values))
    with (args.output_directory/"matrix-dos.csv").open("x",newline="",encoding="ascii") as stream:
        out=csv.writer(stream,lineterminator="\n");out.writerow(["op","matrix","other","result"])
        for index in range(256):
            first=[generator.randrange(-32768,32768) for _ in range(9)]
            second=[generator.randrange(-32768,32768) for _ in range(9)]
            vector=[generator.randrange(-32768,32768) for _ in range(3)]
            if index<3:first=[16384,0,0,0,16384,0,0,0,16384] if index==0 else [0]*9 if index==1 else [-32768]*9
            for name,entry,other,size in [("vector",0x328ee,vector,3),("multiply",0x329f2,second,9),("transpose",0x32a72,[],9)]:
                cpu=machine(image);cpu.mem_write(0x3d770,struct.pack("<9h",*first))
                if name=="vector":
                    cpu.mem_write(0x3db70,struct.pack("<3h",*other));arguments=[0x2400,0x2000,0x2600]
                elif name=="multiply":
                    cpu.mem_write(0x3db70,struct.pack("<9h",*other));arguments=[0x2000,0x2400,0x2600]
                else:arguments=[0x2000,0x2600]
                call(cpu,0x2ea2,entry,arguments)
                result=struct.unpack("<"+"h"*size,cpu.mem_read(0x3dd70,size*2))
                out.writerow([name,words(first),words(other),words(result)])
    with (args.output_directory/"rotations-dos.csv").open("x",newline="",encoding="ascii") as stream:
        out=csv.writer(stream,lineterminator="\n");out.writerow(["axis","angle","matrix"])
        angles=list(range(0,1024,8))+[generator.randrange(65536) for _ in range(128)]
        for angle in angles:
            for axis,entry in [("x",0x36f2a),("y",0x36f80),("z",0x36fd8)]:
                cpu=machine(image);call(cpu,0x36f2,entry,[0x2600,angle]);result=struct.unpack("<9h",cpu.mem_read(0x3dd70,18))
                out.writerow([axis,angle,words(result)])
    with (args.output_directory/"random-dos.csv").open("x",newline="",encoding="ascii") as stream:
        out=csv.writer(stream,lineterminator="\n");out.writerow(["before","result","after"])
        cpu=machine(image)
        seeds=[bytes([0]*6),bytes([255]*6),bytes([0,0,0,0,0,255])]+[bytes(generator.randrange(256) for _ in range(6)) for _ in range(253)]
        # Each independent seed is followed for five steps to check state evolution.
        for seed in seeds:
            for _ in range(5):
                cpu.mem_write(0x3b770+0xa1da,seed)
                call(cpu,0x19dc,0x19e7b,[])
                after=bytes(cpu.mem_read(0x3b770+0xa1da,6))
                out.writerow([seed.hex(),cpu.reg_read(UC_X86_REG_AX),after.hex()]);seed=after
    print("Generated 768 matrix, 768 rotation and 1280 stateful RNG comparisons")


if __name__=="__main__":main()
