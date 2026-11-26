// Mark verified regions without inventing functions at internal block boundaries.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.CodeUnit;

public class MapDrivetrain extends GhidraScript {
    @Override
    public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Unsupported executable SHA256");
        label("1471:359c", "speed_integration_stage", "Boundary after the isolated gearbox prefix. Rust speed_tick now covers the complete update_car_speed function; see docs/speed-grip-numerics.md.");
        label("1471:36d6", "grounded_torque_selection", "Verified block entry: expects BX=carState and the function's BP stack frame. Torque port runs only until 1471:3724; see docs/drivetrain.md.");
        label("1471:3724", "torque_ratio_mass_scaling", "Boundary after torque selection. Original MUL uses the low 16-bit product before SHR; the following division is an unsigned-long helper. Do not assume a signed C expression is equivalent.");
        StructureDataType state = new StructureDataType(new CategoryPath("/Stunts"), "CarRuntimeState", 0xd0);
        String[] names = {"current_rpm_raw", "last_rpm_raw", "idle_rpm2_raw", "speed_difference_raw", "speed_raw", "speed2_raw", "last_speed_raw", "gear_ratio", "gear_ratio_high"};
        for (int i = 0; i < names.length; i++) field(state, 0x22 + i * 2, UnsignedShortDataType.dataType, names[i]);
        field(state, 0x34, ShortDataType.dataType, "knob_x");
        field(state, 0x38, ShortDataType.dataType, "knob_y");
        field(state, 0x3a, ShortDataType.dataType, "target_knob_x");
        field(state, 0x3c, ShortDataType.dataType, "target_knob_y");
        field(state, 0xbc, ByteDataType.dataType, "is_braking");
        field(state, 0xbd, ByteDataType.dataType, "is_accelerating");
        field(state, 0xbe, ByteDataType.dataType, "current_gear");
        field(state, 0xc0, ByteDataType.dataType, "rear_contact_sum");
        field(state, 0xc1, ByteDataType.dataType, "all_contact_sum");
        field(state, 0xc6, ByteDataType.dataType, "engine_limiter_timer");
        field(state, 0xca, ByteDataType.dataType, "changing_gear");
        field(state, 0xcb, ByteDataType.dataType, "shift_timer");
        field(state, 0xcc, ByteDataType.dataType, "transmission_mode");
        currentProgram.getDataTypeManager().addDataType(state, DataTypeConflictHandler.REPLACE_HANDLER);
    }
    private void label(String location, String name, String comment) throws Exception {
        var address = toAddr(location);
        createLabel(address, name, true);
        currentProgram.getListing().setComment(address, CodeUnit.PLATE_COMMENT, comment);
    }
    private void field(StructureDataType structure, int offset, DataType type, String name) {
        structure.replaceAtOffset(offset, type, type.getLength(), name, "Packed CARSTATE field; arithmetic signedness depends on the instruction.");
    }
}
