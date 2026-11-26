// Check steering data, label verified state fields, and define the SIMD layout.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.CodeUnit;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public class MapCarData extends GhidraScript {
    @Override
    public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Unsupported executable SHA256");
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output directory");
        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        int[][] responses = {{8,7,6,5,4,4,3,3,2,2,2,1,1,1,1,1}, {16,14,12,10,8,8,6,6,4,4,4,2,2,1,1,1}};
        try (PrintWriter output = new PrintWriter(Files.newBufferedWriter(directory.resolve("steering-tables.csv"), StandardCharsets.UTF_8))) {
            output.println("fps,speed_bucket,input,delta");
            for (int table = 0; table < 2; table++) {
                var address = toAddr(table == 0 ? "3b77:060a" : "3b77:064a");
                int fps = table == 0 ? 20 : 10;
                for (int bucket = 0; bucket < 16; bucket++) {
                    for (int input = 0; input < 4; input++) {
                        int expected = input == 1 ? responses[table][bucket] : input == 2 ? -responses[table][bucket] : 0;
                        int actual = currentProgram.getMemory().getByte(address.add(bucket * 4 + input));
                        if (actual != expected) throw new IllegalStateException("Steering table mismatch");
                        output.println(fps + "," + bucket + "," + input + "," + actual);
                    }
                }
                createLabel(address, "steering_response_" + fps + "fps", true);
                currentProgram.getListing().setComment(address, CodeUnit.PLATE_COMMENT,
                    "Verified signed byte table: 16 speed buckets, four input codes per bucket. See docs/steering-car-parameters.md.");
            }
        }
        // Locations read by the steering instructions and independently exercised
        // in tools/verify_steering.py. Labels do not alter existing data/code units.
        createLabel(toAddr("3b77:8e96"), "player_steering_angle", true);
        createLabel(toAddr("3b77:8ea2"), "player_speed2", true);
        createLabel(toAddr("3b77:9260"), "frames_per_second", true);
        createLabel(toAddr("3b77:9278"), "steering_response_table_ptr", true);
        createLabel(toAddr("3b77:95ce"), "player_aero_table_far_ptr", true);
        createLabel(toAddr("3b77:95da"), "opponent_aero_table_far_ptr", true);
        createLabel(toAddr("3b77:a6ea"), "player_simulation_parameters", true);
        createLabel(toAddr("3b77:9ed0"), "opponent_simulation_parameters", true);

        // Define a packed model without casting Rust structs over the original file.
        // Unmapped ranges deliberately remain undefined bytes in this Ghidra type.
        StructureDataType simd = new StructureDataType(new CategoryPath("/Stunts"), "CarSimulationParameters", 0x308);
        field(simd, 0, ByteDataType.dataType, "num_gears");
        String[] words = {"mass_raw", "braking_raw", "idle_rpm_raw", "downshift_rpm_raw", "upshift_rpm_raw", "maximum_rpm_raw"};
        for (int i = 0; i < words.length; i++) field(simd, 2 + i * 2, ShortDataType.dataType, words[i]);
        field(simd, 0x0e, new ArrayDataType(UnsignedShortDataType.dataType, 7, 2), "gear_ratios");
        field(simd, 0x1c, new ArrayDataType(ShortDataType.dataType, 14, 2), "gear_knob_xy_pairs");
        field(simd, 0x38, ShortDataType.dataType, "aero_resistance_raw");
        field(simd, 0x3a, ByteDataType.dataType, "idle_torque_raw");
        field(simd, 0x3b, new ArrayDataType(ByteDataType.dataType, 104, 1), "torque_curve");
        field(simd, 0xa4, ShortDataType.dataType, "grip_raw");
        field(simd, 0xb4, ShortDataType.dataType, "sliding_raw");
        field(simd, 0xb6, new ArrayDataType(ShortDataType.dataType, 4, 2), "surface_grip_raw");
        field(simd, 0xc8, new ArrayDataType(ShortDataType.dataType, 4, 2), "collision_xy_pairs");
        field(simd, 0xd0, ShortDataType.dataType, "car_height_raw");
        field(simd, 0xd2, new ArrayDataType(ShortDataType.dataType, 12, 2), "wheel_xyz_triplets");
        field(simd, 0x304, DWordDataType.dataType, "runtime_aero_far_pointer");
        currentProgram.getDataTypeManager().addDataType(simd, DataTypeConflictHandler.REPLACE_HANDLER);
    }

    private void field(StructureDataType structure, int offset, DataType type, String name) {
        structure.replaceAtOffset(offset, type, type.getLength(), name, "Reference SIMD layout; units are not inferred from field names.");
    }
}
