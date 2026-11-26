"""Execute the original RPM helper and bounded gearbox/torque code regions.

Requires Unicorn. Gearbox stops before speed integration at 17CAC; torque runs
only 17DE6..17E34. These fixtures do not imply a full update_car_speed port.
"""
import argparse
import csv
import hashlib
import random
import struct
from pathlib import Path
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BP, UC_X86_REG_BX, UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_IP


def word(data, offset):
    return struct.unpack_from("<H", data, offset)[0]


def simd_file(path):
    data = path.read_bytes()
    count = word(data, 4)
    for index in range(count):
        if data[6 + index * 4:10 + index * 4] == b"simd":
            offset = 6 + count * 8 + struct.unpack_from("<I", data, 6 + count * 4 + index * 4)[0]
            return data[offset:offset + 776]
    raise ValueError("Missing SIMD")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("game_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    binary = args.binary.read_bytes()
    if hashlib.sha256(binary).hexdigest() != "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b":
        raise ValueError("Unsupported executable SHA256")
    image = bytearray(binary[word(binary, 8) * 16:])
    for index in range(word(binary, 6)):
        offset, segment = struct.unpack_from("<HH", binary, word(binary, 24) + index * 4)
        position = segment * 16 + offset
        struct.pack_into("<H", image, position, (word(image, position) + 0x1000) & 0xffff)
    if image[0x7a78:0x7a80] != bytes.fromhex("558bec83ec0a5756"):
        raise ValueError("Gearbox entry mismatch")
    if image[0x9dc6:0x9de8] != bytes.fromhex("558bec558b4e068b4608837e0c007505f7660a8bca3b4e0e73038b4e0e8bc15d5dcb"):
        raise ValueError("RPM helper mismatch")
    def cpu(cs):
        result = Uc(UC_ARCH_X86, UC_MODE_16)
        result.mem_map(0, 1024 * 1024)
        result.mem_write(0x10000, bytes(image))
        for register, value in [(UC_X86_REG_CS, cs), (UC_X86_REG_DS, 0x3b77), (UC_X86_REG_SS, 0x6000), (UC_X86_REG_SP, 0xff00), (UC_X86_REG_BP, 0x7777)]:
            result.reg_write(register, value)
        return result
    def check_stop(machine, address):
        actual = machine.reg_read(UC_X86_REG_CS) * 16 + machine.reg_read(UC_X86_REG_IP)
        if actual != address:
            raise AssertionError(f"Unexpected stop at {actual:x}, expected {address:x}")
    def save(name, header, rows):
        args.output_directory.mkdir(parents=True, exist_ok=True)
        with (args.output_directory / name).open("x", newline="", encoding="ascii") as stream:
            writer = csv.writer(stream, lineterminator="\n")
            writer.writerow(header)
            writer.writerows(rows)
        print(f"{name}: {len(rows)} original-code cases")

    generator = random.Random(0x47454152)
    edges = [0, 1, 8192, 16384, 32768, 65534, 65535]
    rpm_cases = [(current, speed, ratio, changing, idle) for current in [0, 65535] for speed in edges for ratio in edges for changing in [0, 1] for idle in [0, 2000]]
    rpm_cases += [tuple(generator.randrange(65536) for _ in range(5)) for _ in range(128)]
    rpm_rows = []
    for case in rpm_cases:
        machine = cpu(0x19dc)
        machine.mem_write(0x6ff00, struct.pack("<7H", 0xff00, 0xf000, *case))
        machine.emu_start(0x19dc6, 0xfff00, count=100)
        check_stop(machine, 0xfff00)
        if machine.reg_read(UC_X86_REG_BP) != 0x7777 or machine.reg_read(UC_X86_REG_SP) != 0xff04:
            raise AssertionError("RPM helper stack mismatch")
        rpm_rows.append((*case, machine.reg_read(UC_X86_REG_AX)))
    save("rpm-dos.csv", ["current", "speed", "ratio", "changing", "idle", "dos_result"], rpm_rows)

    cars = [(path.name, simd_file(path)) for path in sorted(args.game_directory.glob("CAR*.RES"))]
    gearbox_rows = []
    torque_rows = []
    for name, simd in cars:
        for fps in [10, 20]:
            for transmission in [0, 1]:
                for request in [0, 16, 32, 48]:
                    for phase in range(4):
                        state = bytearray([0xa5] * 208)
                        gear = generator.randrange(simd[0] + 1)
                        state[0xbe] = gear
                        state[0xcc] = transmission
                        state[0xca] = int(phase != 0)
                        state[0xc0] = generator.randrange(2)
                        state[0xc6] = generator.choice([0, 1, 255])
                        state[0xcb] = generator.randrange(256)
                        rpm = generator.choice([0, word(simd, 8) - 1, word(simd, 8), word(simd, 10), word(simd, 10) + 1, word(simd, 12)])
                        for offset, value in [(0x22, rpm), (0x24, generator.randrange(65536)), (0x2c, generator.randrange(65536)), (0x2e, generator.randrange(65536)), (0x30, word(simd, 14 + gear * 2)), (0x32, word(simd, 14 + gear * 2) >> 8)]:
                            struct.pack_into("<H", state, offset, value)
                        x, y = struct.unpack_from("<hh", simd, 28 + gear * 4)
                        neutral_y = struct.unpack_from("<h", simd, 30)[0]
                        for offset, value in [(0x34, x), (0x38, y), (0x3a, x), (0x3c, y)]:
                            struct.pack_into("<h", state, offset, value)
                        if phase == 2:
                            struct.pack_into("<h", state, 0x38, y + 20)
                        if phase == 3:
                            struct.pack_into("<h", state, 0x34, x + 20)
                            struct.pack_into("<h", state, 0x38, neutral_y)
                        before = state.hex()
                        machine = cpu(0x1471)
                        machine.mem_write(0x3d770, simd)
                        machine.mem_write(0x3db70, bytes(state))
                        machine.mem_write(0x449d0, struct.pack("<H", fps))
                        machine.mem_write(0x6ff00, struct.pack("<6H", 0xff00, 0xf000, request, 0, 0x2400, 0x2000))
                        machine.emu_start(0x17a78, 0x17cac, count=2000)
                        check_stop(machine, 0x17cac)
                        after = bytes(machine.mem_read(0x3db70, 208)).hex()
                        gearbox_rows.append((name, request, fps, before, after))
        torque_cases = [(gear, rpm, limiter) for gear in [0, 1, simd[0]] for rpm in [0, 2599, 2600, 4999, 5000, word(simd, 12)] for limiter in [0, 1]]
        torque_cases += [(2, index * 128, 0) for index in range(104)]
        for gear, rpm, limiter in torque_cases:
            machine = cpu(0x1471)
            machine.reg_write(UC_X86_REG_BP, 0xff00)
            # This basic block inherits BX=carState from the grounded-car branch.
            machine.reg_write(UC_X86_REG_BX, 0x2400)
            machine.mem_write(0x3d770, simd)
            state = bytearray(208)
            state[0xbe], state[0xc6] = gear, limiter
            struct.pack_into("<H", state, 0x22, rpm)
            machine.mem_write(0x3db70, bytes(state))
            machine.mem_write(0x6ff06, struct.pack("<4H", 1, 0, 0x2400, 0x2000))
            machine.emu_start(0x17de6, 0x17e34, count=200)
            check_stop(machine, 0x17e34)
            torque_rows.append((name, gear, rpm, limiter, bytes(machine.mem_read(0x6fef6, 1))[0]))
    save("gearbox-dos.csv", ["car", "input", "fps", "before_hex", "after_hex"], gearbox_rows)
    save("torque-dos.csv", ["car", "gear", "rpm", "limiter", "dos_result"], torque_rows)


if __name__ == "__main__":
    main()
