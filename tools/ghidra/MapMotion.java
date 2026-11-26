// Document the bounded physics prefix and preserve its still-unported boundary.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.CodeUnit;

public class MapMotion extends GhidraScript {
    @Override
    public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Unsupported executable SHA256");
        createLabel(toAddr("1471:07be"),"motion_preparation_end",true);
        createLabel(toAddr("1471:0a2e"),"wheel_prediction_end_contact_search_begin",true);
        comment("1471:0692","Entry through 1471:07BE is ported as motion::prepare (512 original-code comparisons). Extended entry through 1471:0A2E is motion::predict_wheels (704 comparisons across eleven cars): upside-down adjustment, suspension attachment displacement, skid rotation and per-wheel travel. Terrain/contact search and final world integration remain unported. Native prototype physics is separate experimental code.");
        comment("1471:07be","motion::prepare boundary; motion::predict_wheels continues through 1471:0A2E. Skid rotation uses mat_rot_zxy(0,0,-angle_z,0). planindex_copy=-1 for the initial four-wheel prediction.");
        comment("1471:0a2e","Original wheel prediction validated in Rust: stack BP-448 predicted positions, BP-374 base positions, BP-240 upside-down adjustment. Stops before the contact retry loop. The experimental desktop solver does not reproduce the remaining original physics.");
        comment("24d6:149a","Pure result ported as matrix::rotation_zxy. 756 original-code comparisons check both composition orders and cached/skipped-axis cases. Oracle initializes the four predefined Y matrices using original mat_rot_y before running.");
        int[] addresses={0x60ce,0x572e,0x571c,0x5748};
        String[] names={"precomputed_y_rotation_0","precomputed_y_rotation_256","precomputed_y_rotation_512","precomputed_y_rotation_768"};
        for(int i=0;i<addresses.length;i++)createLabel(toAddr("3b77:"+Integer.toHexString(addresses[i])),names[i],true);
        StructureDataType view=new StructureDataType(new CategoryPath("/Stunts"),"CarMotionStateView",208);
        field(view,0,new ArrayDataType(DWordDataType.dataType,3,4),"world_position_raw32");
        field(view,12,new ArrayDataType(DWordDataType.dataType,3,4),"previous_world_position_raw32");
        field(view,0x18,new ArrayDataType(ShortDataType.dataType,3,2),"rotation_words");
        field(view,0x1e,ShortDataType.dataType,"pseudo_gravity");
        for(int i=0;i<5;i++)field(view,0x4c+i*8,new ArrayDataType(ShortDataType.dataType,4,2),"reference_rc"+(i+1));
        field(view,0x74,new ArrayDataType(ShortDataType.dataType,12,2),"wheel_world_coordinates_1");
        field(view,0x8c,new ArrayDataType(ShortDataType.dataType,12,2),"wheel_world_coordinates_2");
        currentProgram.getDataTypeManager().addDataType(view,DataTypeConflictHandler.REPLACE_HANDLER);
    }
    private void field(StructureDataType structure,int offset,DataType type,String name){structure.replaceAtOffset(offset,type,type.getLength(),name,"Packed raw state view; remaining physics semantics are still being mapped.");}
    private void comment(String address,String comment)throws Exception{currentProgram.getListing().setComment(toAddr(address),CodeUnit.PLATE_COMMENT,comment);}
}
