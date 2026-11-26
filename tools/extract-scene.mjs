// Extract scene metadata from this project's unchanged reconstructed executable.
// No reference source, Ghidra database or runtime dependency is required.
import fs from 'node:fs';
import crypto from 'node:crypto';
const [input, output] = process.argv.slice(2);
if (!input || !output) throw new Error('Usage: node tools/extract-scene.mjs MCGA.exe OUTPUT.bin');
const exe = fs.readFileSync(input);
if (crypto.createHash('sha256').update(exe).digest('hex') !== '2e2b514d183835267bdfd4ebeeb409b1c9facad78751ebaafeac54d93453e51b') {
    throw new Error('Unsupported executable SHA256');
}
const ds = exe.readUInt16LE(8)*16 + 0x2b770;
const names = [];
for (let i=0;i<116;i++) names.push(exe.subarray(ds+0xa50+i*5,ds+0xa50+i*5+4));
const objects = exe.subarray(ds+0x2098,ds+0x2098+215*14);
const terrain = exe.subarray(ds+0x2c5a,ds+0x2c5a+19*14);
if (names[30].toString('ascii') !== 'road' || objects.readUInt16LE(4*14+4)!==0x78e0) throw new Error('Scene table signature mismatch');
fs.writeFileSync(output,Buffer.concat([...names,objects,terrain]),{flag:'wx'});
console.log('Extracted 116 shape IDs, 215 object records and 19 terrain records');
