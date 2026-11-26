// Describe original scene tables used by the native mesh loader.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.CodeUnit;

public class MapScene extends GhidraScript {
    @Override public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Unsupported executable SHA256");
        StructureDataType record=new StructureDataType(new CategoryPath("/Stunts"),"TrackObjectRecord",0);
        record.add(WordDataType.dataType,"track_info_near",null);
        record.add(WordDataType.dataType,"rotation_y",null);
        record.add(WordDataType.dataType,"shape_near",null);
        record.add(WordDataType.dataType,"low_shape_near",null);
        for(String name:new String[]{"overlay_id","paint_job","z_bias_flags","multi_tile_flags","physical_model","reserved"})
            record.add(ByteDataType.dataType,name,null);
        DataType type=currentProgram.getDataTypeManager().addDataType(record,DataTypeConflictHandler.REPLACE_HANDLER);
        createLabel(toAddr("3b77:0a50"),"scene_shape_ids_116",true);
        createLabel(toAddr("3b77:2098"),"track_object_records_215",true);
        createLabel(toAddr("3b77:2c5a"),"terrain_object_records_19",true);
        createLabel(toAddr("3b77:764c"),"runtime_shape_array_stride_22",true);
        getFunctionAt(toAddr("1e1a:38ba")).setName("subst_hillroad_track",ghidra.program.model.symbol.SourceType.USER_DEFINED);
        currentProgram.getListing().setComment(toAddr("1e1a:38ba"),CodeUnit.PLATE_COMMENT,
            "Complete Rust scene::hillroad port validated by execution of all 65536 byte input pairs. Maps terrain 7..10 and compatible straight roads/ramps to internal scene IDs B6..C5; returns zero for other pairs. Used before rendering slope tiles, replacing the terrain and road with a dedicated inclined mesh.");
        clearListing(toAddr("3b77:2098"),toAddr("3b77:2d63"));
        createData(toAddr("3b77:2098"),new ArrayDataType(type,215,14));
        createData(toAddr("3b77:2c5a"),new ArrayDataType(type,19,14));
        currentProgram.getListing().setComment(toAddr("3b77:2098"),CodeUnit.PLATE_COMMENT,
            "Extracted unchanged by tools/extract-scene.mjs. 14-byte object records; near shape pointers index runtime SHAPE3D at DS:764C with stride 22. Names are 116 five-byte strings at DS:0A50. Multi bit 0 spans runtime rows (raw file row y-1 through y); bit 1 spans columns x through x+1. Meshes are in original GAME1/GAME2.P3S archives. Native scene triangle contacts are experimental, not original collision planes.");
        currentProgram.getListing().setComment(toAddr("3b77:2c5a"),CodeUnit.PLATE_COMMENT,
            "19 terrain records. Entry zero is a fence helper and is not submitted for blank terrain. Plateau ID 6 uses 450 original units. Runtime trackrows[i]=30*(29-i), terrainrows[i]=30*i; trackcenterpos[i]=(29-i)*1024+512. Raw element row y therefore has positive world Z y*1024+512, so do not mirror local mesh Z.");
    }
}
