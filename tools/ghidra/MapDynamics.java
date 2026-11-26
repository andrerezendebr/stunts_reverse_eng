// Annotate CPU-verified speed/grip ports and define their numerical data views.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.CodeUnit;

public class MapDynamics extends GhidraScript {
    @Override
    public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Unsupported executable SHA256");
        createLabel(toAddr("3b77:4c84"),"sine_quarter_wave_q14",true);
        createLabel(toAddr("3b77:a1da"),"kevinrandom_state_6bytes",true);
        createLabel(toAddr("3b77:95d2"),"element_map_far_pointer",true);
        createLabel(toAddr("3b77:8c3c"),"terrain_row_offsets",true);
        comment("1471:3368","Complete Rust port: drivetrain::speed_tick. Checked against 1408 isolated states and 5720 sequential frames, including all 208 state bytes and global top speed. World position is not integrated here.");
        comment("1471:38c6","Complete Rust port: grip::tick. Third argument selects detailed/non-detailed paths; player_op passes 1, despite the misleading reference argument name. 1584 original-code comparisons include surface and banked-marker cases.");
        comment("2ea2:3ece","Matrix-vector transform: each signed product is shifted by 14 before word-sized accumulation, without multiply_and_scale rounding. Rust port covers distinct input/output storage.");
        comment("2ea2:3fd2","mat_multiply(right,left,out) computes left * right in column-major storage. Products truncate separately; input/output alias side effects are outside the pure Rust API.");
        comment("2ea2:4052","Reference name mat_invert actually transposes the matrix. It is not a general numerical matrix inverse.");
        StructureDataType vector=new StructureDataType(new CategoryPath("/Stunts"),"Vector3i16",6);
        for(int i=0;i<3;i++)field(vector,i*2,ShortDataType.dataType,new String[]{"x","y","z"}[i]);
        StructureDataType matrix=new StructureDataType(new CategoryPath("/Stunts"),"Matrix3Q14",18);
        for(int c=0;c<3;c++)for(int r=0;r<3;r++)field(matrix,(c*3+r)*2,ShortDataType.dataType,"m"+(r+1)+(c+1));
        StructureDataType grip=new StructureDataType(new CategoryPath("/Stunts"),"CarGripStateView",208);
        int[] offsets={0x18,0x1a,0x1c,0x1e,0x20,0x36,0x3e,0x40,0x42,0x44,0x46};
        String[] names={"rotation_x","rotation_y","rotation_z","pseudo_gravity","steering_angle","slip_angle","angle_z","front_wheel_angle","residual","demanded_grip","available_grip"};
        for(int i=0;i<offsets.length;i++)field(grip,offsets[i],ShortDataType.dataType,names[i]);
        field(grip,0xc2,new ArrayDataType(ByteDataType.dataType,4,1),"wheel_surface_codes");
        field(grip,0xc7,ByteDataType.dataType,"sliding_flag");
        field(grip,0xc9,ByteDataType.dataType,"crash_flag");
        field(grip,0xcf,ByteDataType.dataType,"state_flags");
        for(DataType type:new DataType[]{vector,matrix,grip})currentProgram.getDataTypeManager().addDataType(type,DataTypeConflictHandler.REPLACE_HANDLER);
    }
    private void field(StructureDataType structure,int offset,DataType type,String name){structure.replaceAtOffset(offset,type,type.getLength(),name,"Verified numerical layout; units remain raw game units.");}
    private void comment(String address,String comment)throws Exception{currentProgram.getListing().setComment(toAddr(address),CodeUnit.PLATE_COMMENT,comment);}
}
