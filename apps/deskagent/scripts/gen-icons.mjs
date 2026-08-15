// Generate minimal placeholder PNG icons for the Tauri bundle config.
// DeskAgent brand color: #287AF7 on a 40-alpha background. 32x32 and 128x128.
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons");
mkdirSync(dir, { recursive: true });

function crc32(buf) {
  let crc = 0xffffffff;
  for (const b of buf) {
    crc ^= b;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function png(size) {
  const w = size;
  const h = size;
  const stride = w * 4 + 1;
  const raw = Buffer.alloc(stride * h);
  for (let y = 0; y < h; y++) {
    raw[y * stride] = 0; // filter: none
    for (let x = 0; x < w; x++) {
      const o = y * stride + 1 + x * 4;
      raw[o] = 40;
      raw[o + 1] = 122;
      raw[o + 2] = 247;
      raw[o + 3] = 255;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  return Buffer.concat([sig, chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

writeFileSync(path.join(dir, "32x32.png"), png(32));
writeFileSync(path.join(dir, "128x128.png"), png(128));
// .ico/.icns are real formats, but Tauri accepts PNG bytes for bundling on Linux;
// macOS/Windows release bundling should replace these with proper icons.
writeFileSync(path.join(dir, "icon.ico"), png(32));
writeFileSync(path.join(dir, "icon.icns"), png(128));
console.log("placeholder icons written to", dir);
