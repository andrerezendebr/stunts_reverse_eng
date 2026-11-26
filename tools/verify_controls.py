"""Compare speed, steering, and grip in sequence; world position is not integrated."""
import argparse
import csv
import struct
from pathlib import Path
from dos_machine import load_image, machine, call
from verify_drivetrain import simd_file,word


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary",type=Path)
    parser.add_argument("game_directory",type=Path)
    parser.add_argument("output",type=Path)
    args=parser.parse_args()
    image=load_image(args.binary)
    tables={}
    with (Path(__file__).resolve().parent.parent/"stunts-rs/tests/fixtures/car-aero-dos.csv").open() as stream:
        for row in csv.DictReader(stream):
            if row["side"]=="0":tables.setdefault(row["car"],[0]*64)[int(row["index"])]=int(row["dos_result"])
    total=0
    with args.output.open("x",newline="",encoding="ascii") as stream:
        writer=csv.writer(stream,lineterminator="\n")
        writer.writerow(["car","fps","surface","frame","input","top_after","after_hex"])
        for path in sorted(args.game_directory.glob("CAR*.RES")):
            simd=bytearray(simd_file(path))
            struct.pack_into("<HH",simd,772,0,0x8000)
            for fps in [10,20]:
                for surface in [1,3]:
                    cpu=machine(image)
                    state=bytearray(208)
                    state[2],state[10]=3,4
                    state[0xbe],state[0xc0],state[0xc1],state[0xcc]=1,1,1,1
                    state[0xc2:0xc6]=bytes([surface]*4)
                    for offset,value in [(0x22,word(simd,6)),(0x30,word(simd,16)),(0x32,word(simd,16)>>8)]:struct.pack_into("<H",state,offset,value)
                    point=simd[32:36]
                    state[0x34:0x36],state[0x38:0x3a],state[0x3a:0x3e]=point[:2],point[2:],point
                    # Steering reads the real global player state, so all three
                    # calls use its actual DS:8E76 address in this comparison.
                    cpu.mem_write(0x3b770+0x8e76,bytes(state))
                    cpu.mem_write(0x3d770,bytes(simd))
                    cpu.mem_write(0x80000,struct.pack("<64h",*tables[path.name]))
                    cpu.mem_write(0x3b770+0x9260,struct.pack("<H",fps))
                    cpu.mem_write(0x3b770+0x9278,struct.pack("<H",0x64a if fps==10 else 0x60a))
                    cpu.mem_write(0x3b770+0x8e72,b"\0\0")
                    cpu.mem_write(0x3b770+0x8c3c,struct.pack("<30H",*[i*30 for i in range(30)]))
                    cpu.mem_write(0x3b770+0x95d2,struct.pack("<HH",0,0x9000))
                    for frame in range(150):
                        request=1 if frame<50 else 5 if frame<80 else 9 if frame<100 else 0 if frame<120 else 2
                        call(cpu,0x1471,0x17a78,[request,0,0x8e76,0x2000])
                        call(cpu,0x1471,0x187b2,[(request>>2)&3])
                        call(cpu,0x1471,0x17fd6,[0x8e76,0x2000,1])
                        after=bytes(cpu.mem_read(0x3b770+0x8e76,208)).hex()
                        top=struct.unpack("<H",cpu.mem_read(0x3b770+0x8e72,2))[0]
                        writer.writerow([path.name,fps,surface,frame,request,top,after])
                        total+=1
    print(f"Executed {total} sequential three-kernel frames for 11 cars, two FPS settings, and two surfaces")


if __name__=="__main__":main()
