import { writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { deflateSync } from 'node:zlib'

const __dirname = dirname(fileURLToPath(import.meta.url))
const outputPath = resolve(__dirname, '../public/og-image.png')

// CRC32 implementation (needed for PNG chunk validation)
const crcTable = new Uint32Array(256)
for (let i = 0; i < 256; i++) {
  let c = i
  for (let j = 0; j < 8; j++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  crcTable[i] = c
}
function crc32(buf) {
  let crc = 0xffffffff
  for (const byte of buf) crc = crcTable[(crc ^ byte) & 0xff] ^ (crc >>> 8)
  const result = Buffer.allocUnsafe(4)
  result.writeUInt32BE((crc ^ 0xffffffff) >>> 0, 0)
  return result
}

function chunk(type, data) {
  const typeBuf = Buffer.from(type, 'ascii')
  const lenBuf = Buffer.allocUnsafe(4)
  lenBuf.writeUInt32BE(data.length, 0)
  return Buffer.concat([lenBuf, typeBuf, data, crc32(Buffer.concat([typeBuf, data]))])
}

const W = 1200
const H = 630

// IHDR: width, height, bit depth 8, color type 2 (RGB)
const ihdr = Buffer.allocUnsafe(13)
ihdr.writeUInt32BE(W, 0)
ihdr.writeUInt32BE(H, 4)
ihdr[8] = 8
ihdr[9] = 2
ihdr[10] = 0
ihdr[11] = 0
ihdr[12] = 0

// Pixel data: filter byte (0=None) + RGB for each row
const raw = Buffer.allocUnsafe(H * (1 + W * 3))
for (let y = 0; y < H; y++) {
  const rowOff = y * (1 + W * 3)
  raw[rowOff] = 0 // filter: none
  for (let x = 0; x < W; x++) {
    const p = rowOff + 1 + x * 3
    // Dark slate background #0f172a with green top bar on first 6 rows
    if (y < 6) {
      raw[p] = 0x22
      raw[p + 1] = 0xc5
      raw[p + 2] = 0x5e // #22c55e green
    } else {
      raw[p] = 0x0f
      raw[p + 1] = 0x17
      raw[p + 2] = 0x2a // #0f172a dark slate
    }
  }
}

const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), // PNG signature
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 1 })),
  chunk('IEND', Buffer.alloc(0)),
])

await writeFile(outputPath, png)
console.info(`og-image.png created (${W}x${H}, ${png.length} bytes)`)
