// Apply the first manually checked names to this exact reconstructed image.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.listing.CodeUnit;

public class AnnotateStartup extends GhidraScript {
    @Override
    public void run() throws Exception {
        String expected = "2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b";
        if (!expected.equalsIgnoreCase(currentProgram.getExecutableSHA256())) {
            throw new IllegalArgumentException("This script only supports the recorded MCGA.exe SHA256");
        }
        name("1000:0000", "stunts_main", "Reference: ReStunts asmorig/seg000.asm ported_stuntsmain_. Opening instructions verified: init_main, init_div0, then 30-row track and terrain lookup initialization. This is game main, not the DOS entry point.");
        name("39e1:0046", "init_main", "Reference-derived name; verified far-call target from the opening instructions of stunts_main.");
        name("19dc:0128", "init_div0", "Reference-derived name; verified far-call target from the opening instructions of stunts_main. Full semantics still require review.");
    }

    private void name(String address, String name, String comment) throws Exception {
        var location = toAddr(address);
        var function = getFunctionAt(location);
        if (function == null) throw new IllegalStateException("Missing function at " + address);
        function.setName(name, SourceType.USER_DEFINED);
        currentProgram.getListing().setComment(location, CodeUnit.PLATE_COMMENT, comment);
    }
}
