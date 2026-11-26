"""Execute complete update_car_speed for isolated and multi-frame comparisons.

Aero tables are seeded from previously verified original-code fixtures. Engine,
gearbox, RPM and long-arithmetic helpers execute as original DOS instructions.
"""
import argparse
import csv
import random
import struct
from pathlib import Path
from dos_machine import load_image, machine, call
from verify_drivetrain import simd_file, word


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("game_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    image = load_image(args.binary)
    tables = {}
    with (Path(__file__).resolve().parent.parent / "stunts-rs/tests/fixtures/car-aero-dos.csv").open() as stream:
        for row in csv.DictReader(stream):
            if row["side"] == "0":
                tables.setdefault(row["car"], [0] * 64)[int(row["index"])] = int(row["dos_result"])
    cars = [(path.name, simd_file(path)) for path in sorted(args.game_directory.glob("CAR*.RES"))]
    def prepare(name, simd, state, fps, oppspeed, top):
        cpu = machine(image)
        configured = bytearray(simd)
        struct.pack_into("<HH", configured, 772, 0, 0x8000)
        cpu.mem_write(0x3d770, bytes(configured))
        cpu.mem_write(0x3db70, bytes(state))
        cpu.mem_write(0x80000, struct.pack("<64h", *tables[name]))
        cpu.mem_write(0x3b770 + 0x9260, struct.pack("<H", fps))
        cpu.mem_write(0x3b770 + 0x95e0, bytes([oppspeed]))
        cpu.mem_write(0x3b770 + 0x8e72, struct.pack("<H", top))
        return cpu
    def initial(simd, gear=1):
        state = bytearray(208)
        state[0xbe], state[0xc0], state[0xc1], state[0xcc] = gear, 1, 1, 1
        ratio = word(simd, 14 + gear * 2)
        for offset, value in [(0x22, word(simd, 6)), (0x30, ratio), (0x32, ratio >> 8)]:
            struct.pack_into("<H", state, offset, value)
        point = simd[28 + gear * 4:32 + gear * 4]
        state[0x34:0x36], state[0x38:0x3a], state[0x3a:0x3e] = point[:2], point[2:], point
        return state
    def result(cpu):
        return bytes(cpu.mem_read(0x3db70, 208)).hex(), struct.unpack("<H", cpu.mem_read(0x3b770 + 0x8e72, 2))[0]
    generator = random.Random(0x5350454544)
    args.output_directory.mkdir(parents=True, exist_ok=True)
    with (args.output_directory / "speed-dos.csv").open("x", newline="", encoding="ascii") as stream:
        output = csv.writer(stream, lineterminator="\n")
        output.writerow(["car", "input", "fps", "opponent", "opponent_speed", "top_before", "before_hex", "top_after", "after_hex"])
        count = 0
        for name, simd in cars:
            for fps in [10, 20]:
                for opponent in [0, 1]:
                    for throttle in range(4):
                        for phase in range(8):
                            gear = generator.randrange(simd[0] + 1)
                            state = initial(simd, gear)
                            state[0xcc] = phase % 2
                            state[0xca] = int(phase == 3)
                            state[0xc0] = int(phase != 2)
                            state[0xc1] = int(phase != 2)
                            state[0xc6] = generator.choice([0, 1, 5, 30, 255])
                            state[0xbc], state[0xbd] = 0x5a, 0xa5
                            rpm = generator.choice([0, 2599, 5000, word(simd, 12), word(simd, 12) + 1])
                            speed = generator.choice([0, 1, 1023, 1024, 32767, 32768, 62720, 64000, 65535])
                            speed2 = speed if phase != 4 else generator.randrange(65536)
                            gravity = generator.choice([0, 130, -130, -32768, 32767])
                            for offset, value in [(0x1e, gravity & 0xffff), (0x22, rpm), (0x2a, speed), (0x2c, speed2), (0x2e, generator.randrange(65536))]:
                                struct.pack_into("<H", state, offset, value)
                            if phase == 5: struct.pack_into("<H", state, 0x32, 65535)
                            request = throttle | generator.choice([0, 16, 32, 48])
                            oppspeed = generator.choice([0, 100, 199, 200, 201, 255])
                            top = generator.randrange(65536)
                            cpu = prepare(name, simd, state, fps, oppspeed, top)
                            call(cpu, 0x1471, 0x17a78, [request, opponent, 0x2400, 0x2000])
                            after, top_after = result(cpu)
                            output.writerow([name, request, fps, opponent, oppspeed, top, state.hex(), top_after, after])
                            count += 1
        print(f"speed-dos.csv: {count} complete original-function cases")
    with (args.output_directory / "speed-traces-dos.csv").open("x", newline="", encoding="ascii") as stream:
        output = csv.writer(stream, lineterminator="\n")
        output.writerow(["car", "fps", "frame", "input", "top_after", "after_hex"])
        count = 0
        for name, simd in cars:
            for fps in [10, 20]:
                cpu = prepare(name, simd, initial(simd), fps, 200, 0)
                for frame in range(260):
                    request = 1 if frame < 180 else 0 if frame < 220 else 2
                    call(cpu, 0x1471, 0x17a78, [request, 0, 0x2400, 0x2000])
                    after, top_after = result(cpu)
                    output.writerow([name, fps, frame, request, top_after, after])
                    count += 1
        print(f"speed-traces-dos.csv: {count} sequential complete-function frames")


if __name__ == "__main__":
    main()
