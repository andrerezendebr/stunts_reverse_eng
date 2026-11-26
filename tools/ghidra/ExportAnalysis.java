// Export a reproducible inventory and a small decompilation sample.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.mem.MemoryBlock;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public class ExportAnalysis extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output directory");
        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        String name = currentProgram.getName();
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(directory.resolve(name + "-summary.txt"), StandardCharsets.UTF_8))) {
            out.println("Program: " + name);
            out.println("Language: " + currentProgram.getLanguageID());
            out.println("Image base: " + currentProgram.getImageBase());
            out.println("Functions: " + currentProgram.getFunctionManager().getFunctionCount());
            out.println("Instructions: " + currentProgram.getListing().getNumInstructions());
            out.println("Memory blocks:");
            for (MemoryBlock block : currentProgram.getMemory().getBlocks()) {
                out.println(block.getName() + " " + block.getStart() + " " + block.getEnd() + " size=" + block.getSize());
            }
            out.println("External entry points:");
            var entries = currentProgram.getSymbolTable().getExternalEntryPointIterator();
            while (entries.hasNext()) out.println(entries.next());
        }
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(directory.resolve(name + "-functions.tsv"), StandardCharsets.UTF_8))) {
            out.println("address\tname\tbody_bytes");
            FunctionIterator functions = currentProgram.getFunctionManager().getFunctions(true);
            while (functions.hasNext()) {
                Function function = functions.next();
                out.println(function.getEntryPoint() + "\t" + function.getName() + "\t" + function.getBody().getNumAddresses());
            }
        }
        DecompInterface decompiler = new DecompInterface();
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(directory.resolve(name + "-sample.c"), StandardCharsets.UTF_8))) {
            out.println("/* Automatically generated Ghidra pseudocode. Types and function boundaries are not verified. */");
            decompiler.openProgram(currentProgram);
            FunctionIterator functions = currentProgram.getFunctionManager().getFunctions(true);
            for (int i = 0; i < 12 && functions.hasNext(); i++) {
                Function function = functions.next();
                out.println("/* " + function.getEntryPoint() + " */");
                var result = decompiler.decompileFunction(function, 20, monitor);
                if (result.decompileCompleted()) out.println(result.getDecompiledFunction().getC());
                else out.println("/* " + result.getErrorMessage() + " */");
            }
        } finally {
            decompiler.dispose();
        }
    }
}
