"""Shared optional CPU-oracle setup; not a runtime dependency of the Rust tools."""
import hashlib
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16
from unicorn.x86_const import UC_X86_REG_BP, UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_IP


def load_image(path):
    binary = path.read_bytes()
    if hashlib.sha256(binary).hexdigest() != "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b":
        raise ValueError("Unsupported executable SHA256")
    word = lambda offset: struct.unpack_from("<H", binary, offset)[0]
    image = bytearray(binary[word(8) * 16:])
    for index in range(word(6)):
        offset, segment = struct.unpack_from("<HH", binary, word(24) + index * 4)
        position = segment * 16 + offset
        struct.pack_into("<H", image, position, (struct.unpack_from("<H", image, position)[0] + 0x1000) & 0xffff)
    return bytes(image)


def machine(image):
    result = Uc(UC_ARCH_X86, UC_MODE_16)
    result.mem_map(0, 1024 * 1024)
    result.mem_write(0x10000, image)
    result.reg_write(UC_X86_REG_DS, 0x3b77)
    result.reg_write(UC_X86_REG_SS, 0x6000)
    return result


def call(cpu, segment, entry, arguments, limit=20000):
    cpu.reg_write(UC_X86_REG_CS, segment)
    cpu.reg_write(UC_X86_REG_SP, 0xff00)
    cpu.reg_write(UC_X86_REG_BP, 0x7777)
    cpu.mem_write(0x6ff00, struct.pack("<" + "H" * (2 + len(arguments)), 0xff00, 0xf000, *arguments))
    cpu.emu_start(entry, 0xfff00, count=limit)
    if cpu.reg_read(UC_X86_REG_CS) != 0xf000 or cpu.reg_read(UC_X86_REG_IP) != 0xff00:
        raise AssertionError("DOS function did not return within the instruction limit")
    if cpu.reg_read(UC_X86_REG_BP) != 0x7777 or cpu.reg_read(UC_X86_REG_SP) != 0xff04:
        raise AssertionError("DOS function did not restore its stack frame")
