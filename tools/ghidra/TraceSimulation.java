// Check and name simulation entry points, then export their calls and pseudocode.
// @category Stunts
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.data.ShortDataType;
import ghidra.program.model.data.UnsignedShortDataType;
import ghidra.program.model.symbol.SourceType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HexFormat;

public class TraceSimulation extends GhidraScript {
    @Override
    public void run() throws Exception {
        if (!"2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b".equalsIgnoreCase(currentProgram.getExecutableSHA256())) {
            throw new IllegalArgumentException("Unsupported executable SHA256");
        }
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output directory");
        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        String[][] targets = {
            {"1471:28f8", "update_gamestate", "558bec83ec0457568b1e648ec43ebe9e268a018846fc0ac07405"},
            {"1471:2ad8", "player_op", "558bec83ec525756803e2f92007404fe0e2f92c606458f01803e"},
            {"1471:3368", "update_car_speed", "558bec83ec0a5756833e6092147507c746fe0600eb05c746fe0c"},
            {"1e1a:2534", "track_setup", "558bec81ecf00a5756b88003995250b8a82f50"},
            {"21b7:000a", "run_game", "558bec83ec1656c746f4ffffc706c4810000c706c6814001c746"},
            {"2ea2:1624", "multiply_and_scale", "558bec8b4606f76e08d1e0d1d2d1e0d1d2d1e083d2008bc25dcb"},
            {"1471:40a2", "update_steering", "558bec83ec0657568b3e968ea1a28eb10ad3e824fc8846fc988b"},
            {"1471:5592", "setup_aero_trackdata", "558bec83ec025756837e0a007403e98300"},
            {"274b:15dc", "locate_shape_alt", "558becff760aff7608ff7606"},
            {"19dc:0006", "update_rpm_from_speed", "558bec558b4e068b4608837e0c007505f7660a8bca3b4e0e73038b4e0e8bc15d5dcb"},
            {"1471:38c6", "update_grip", "558bec83ec1057568b5e0680bfc100007514c7474000008b"},
            {"2ea2:3cbe", "sin_fast", "558bec8b46068adc32e483e303d1e3"},
            {"2ea2:3d0c", "cos_fast", "558bec8b4606050001ebad"},
            {"2ea2:3ece", "mat_mul_vector", "558bec56578b5e088b76068b7e0a33d28b070bc074108b0c"},
            {"2ea2:3fd2", "mat_multiply", "558bec56578b76068b5e088b7e0ab9090033d28b040bc074"},
            {"2ea2:4052", "mat_transpose", "558bec56578b76068b7e08b909003bf7751f8b4402874406"},
            {"36f2:000a", "mat_rot_x", "558bec83ec04ff7608"},
            {"36f2:0060", "mat_rot_y", "558bec83ec04ff7608"},
            {"36f2:00b8", "mat_rot_z", "558bec83ec04ff7608"},
            {"19dc:00bb", "get_kevinrandom", "a0dfa10206dea1a2dea10206dda1a2dda10206dca1a2dca1"},
            {"1471:0692", "update_player_state", "558bec81ece40157568b5e068b078b5702a34e928916509289470c89570e8b5e"},
            {"24d6:149a", "mat_rot_zxy", "558bec83ec0457562bf6"}
        };
        // Guard both the file version and each instruction prefix before renaming.
        // Prefixes stop before relocated segment words, which Ghidra adjusts.
        for (String[] target : targets) {
            var address = toAddr(target[0]);
            byte[] expected = HexFormat.of().parseHex(target[2]);
            byte[] actual = new byte[expected.length];
            currentProgram.getMemory().getBytes(address, actual);
            if (!java.util.Arrays.equals(expected, actual)) throw new IllegalStateException("Byte mismatch at " + address);
            Function function = getFunctionAt(address);
            if (function == null) throw new IllegalStateException("Missing function at " + address);
            function.setName(target[1], SourceType.USER_DEFINED);
            currentProgram.getListing().setComment(address, CodeUnit.PLATE_COMMENT,
                "Name matched to ReStunts reference disassembly; exact instruction prefix checked. See docs/speed-grip-numerics.md and earlier mapping notes for scope and verification.");
        }
        // The leaf arithmetic routine reads two signed words at BP+6 and BP+8.
        // Fix its verified signature so callers no longer inherit guessed arity.
        Function multiply = getFunctionAt(toAddr("2ea2:1624"));
        multiply.setReturnType(ShortDataType.dataType, SourceType.USER_DEFINED);
        multiply.replaceParameters(Function.FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS, true,
            SourceType.USER_DEFINED,
            new ParameterImpl("a", ShortDataType.dataType, currentProgram),
            new ParameterImpl("b", ShortDataType.dataType, currentProgram));
        Function rpm = getFunctionAt(toAddr("19dc:0006"));
        rpm.setReturnType(UnsignedShortDataType.dataType, SourceType.USER_DEFINED);
        rpm.replaceParameters(Function.FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS, true,
            SourceType.USER_DEFINED,
            new ParameterImpl("current_rpm", UnsignedShortDataType.dataType, currentProgram),
            new ParameterImpl("speed", UnsignedShortDataType.dataType, currentProgram),
            new ParameterImpl("gear_ratio", UnsignedShortDataType.dataType, currentProgram),
            new ParameterImpl("changing_gear", UnsignedShortDataType.dataType, currentProgram),
            new ParameterImpl("idle_rpm", UnsignedShortDataType.dataType, currentProgram));
        DecompInterface decompiler = new DecompInterface();
        try (PrintWriter calls = new PrintWriter(Files.newBufferedWriter(directory.resolve("simulation-calls.tsv"), StandardCharsets.UTF_8))){
            calls.println("caller_address\tcaller_name\tcallee_address\tcallee_name");
            decompiler.openProgram(currentProgram);
            for (String[] target : targets) {
                Function function = getFunctionAt(toAddr(target[0]));
                var callees = new ArrayList<>(function.getCalledFunctions(monitor));
                callees.sort(Comparator.comparing(Function::getEntryPoint));
                for (Function callee : callees) calls.println(function.getEntryPoint() + "\t" + function.getName() + "\t" + callee.getEntryPoint() + "\t" + callee.getName());
                try (PrintWriter output = new PrintWriter(Files.newBufferedWriter(directory.resolve(target[1] + ".c"), StandardCharsets.UTF_8))) {
                    output.println("/* Ghidra pseudocode; inferred types and control flow still need review. */");
                    var result = decompiler.decompileFunction(function, 30, monitor);
                    if (!result.decompileCompleted()) throw new IllegalStateException(result.getErrorMessage());
                    output.println(result.getDecompiledFunction().getC());
                }
            }
        } finally { decompiler.dispose(); }
    }
}
