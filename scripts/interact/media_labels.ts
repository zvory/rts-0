// Small portable bitmap headings; FFmpeg builds need neither drawtext nor system fonts.
import fs from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { PNG } = require("pngjs");
const ROWS: Record<string, string> = {
  A:"0e11111f111111", B:"1e11111e11111e", C:"0e11101010110e", D:"1e11111111111e",
  E:"1f10101e10101f", F:"1f10101e101010", G:"0e11101711110f", H:"1111111f111111",
  I:"0e04040404040e", J:"0702020212120c", K:"11121418141211", L:"1010101010101f",
  M:"111b1515111111", N:"11191513111111", O:"0e11111111110e", P:"1e11111e101010",
  Q:"0e11111115120d", R:"1e11111e141211", S:"0f10100e01011e", T:"1f040404040404",
  U:"1111111111110e", V:"11111111110a04", W:"11111115151b11", X:"11110a040a1111",
  Y:"11110a04040404", Z:"1f01020408101f", "0":"0e11131519110e", "1":"040c040404040e",
  "2":"0e11010204081f", "3":"1e01010e01011e", "4":"02060a121f0202", "5":"1f10101e01011e",
  "6":"0e10101e11110e", "7":"1f010204080808", "8":"0e11110e11110e", "9":"0e11110f01010e",
  " ":"00000000000000", ".":"00000000000c0c", "_":"0000000000001f", "-":"0000001f000000",
  "+":"0004041f040400", "/":"01010204081010", ":":"000c0c000c0c00", "(":"02040808080402", ")":"08040202020408",
};
export function writeMediaLabel(file: string, text: string, width: number) {
  const height = 48, png = new PNG({ width, height });
  for (let i = 0; i < png.data.length; i += 4) { png.data[i] = 16; png.data[i+1] = 16; png.data[i+2] = 16; png.data[i+3] = 255; }
  const scale = width >= 400 ? 2 : 1, columns = Math.floor((width - 16) / (6 * scale));
  for (const [index, letter] of [...text.toUpperCase()].entries()) {
    const row = Math.floor(index / columns), x = 8 + index % columns * 6 * scale, y = 6 + row * 9 * scale;
    const bits = ROWS[letter];
    if (!bits || row > 1) continue;
    for (let gy = 0; gy < 7; gy++) for (let gx = 0; gx < 5; gx++) {
      if (!(parseInt(bits.slice(gy*2, gy*2+2), 16) & (1 << (4-gx)))) continue;
      for (let dy = 0; dy < scale; dy++) for (let dx = 0; dx < scale; dx++) {
        const offset = ((y+gy*scale+dy)*width+x+gx*scale+dx)*4;
        png.data[offset] = png.data[offset+1] = png.data[offset+2] = 255;
      }
    }
  }
  fs.writeFileSync(file, PNG.sync.write(png), { mode: 0o600 });
}
