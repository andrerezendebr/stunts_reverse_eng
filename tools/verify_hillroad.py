"""Exhaustively execute original slope-road substitution for all byte pairs."""
import argparse
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_AX
from dos_machine import load_image, machine, call

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary",type=Path)
parser.add_argument("output",type=Path)
args=parser.parse_args()
image=load_image(args.binary)
if image[0x11a5a:0x11a65]!=bytes.fromhex("558bec8a46062ae43d0700"):
    raise ValueError("Slope substitution entry mismatch")
cpu=machine(image)
results=bytearray()
for terrain in range(256):
    for element in range(256):
        call(cpu,0x1e1a,0x21a5a,[terrain,element])
        result=cpu.reg_read(UC_X86_REG_AX)
        if result>255:raise AssertionError("Unexpected non-byte result")
        results.append(result)
with args.output.open("xb") as stream:stream.write(results)
print("Executed all 65536 original terrain/element substitution pairs")
