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
        comment("1471:0692","Only entry through 1471:07BE is ported as motion::prepare: previous-position copy, speed scale, rotation preparation and pseudo-gravity. 512 original-code comparisons. Remaining suspension/contact/world integration is not ported.");
        comment("1471:07be","Rust motion::prepare stops before this instruction. Remaining update_player_state includes skid rotation, wheel geometry, terrain/contact queries and world integration.");
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
