/**
 * Binary decoding for initialization and territory data.
 * Optimized for minimal allocations and fast parsing.
 */

import { RgbColor } from "@/shared/render";
import { BinaryMessageType } from "@/shared/api/messages";

/**
 * Decode binary message envelope to extract message type and payload.
 *
 * Format: [type:1][payload:N]
 * - type: 0 = Init, 1 = Delta
 * - payload: remaining bytes (zero-copy slice)
 *
 * Returns null if the envelope is invalid or has an unknown message type.
 */
export function decodeBinaryEnvelope(
    data: Uint8Array,
): { type: BinaryMessageType; payload: Uint8Array } | null {
    if (data.length < 1) {
        console.error("Binary envelope too short (need at least 1 byte)");
        return null;
    }

    const type = data[0];
    if (type !== BinaryMessageType.Init && type !== BinaryMessageType.Delta) {
        console.error(`Unknown binary message type: ${type}`);
        return null;
    }

    // Zero-copy slice of payload (no allocation)
    return {
        type,
        payload: data.subarray(1),
    };
}

/**
 * Decoded territory snapshot using parallel arrays for zero-allocation parsing.
 * This avoids creating millions of temporary objects which would trigger GC.
 */
interface DecodedSnapshot {
    indices: Uint32Array;
    ownerIds: Uint16Array;
}

export interface TerrainData {
    width: number;
    height: number;
    tileIds: Uint8Array;
    palette: Array<RgbColor>;
}

export interface DecodedInitBinary {
    terrain: TerrainData;
    territory: DecodedSnapshot;
    nationPalette: Array<RgbColor>;
}

/**
 * Decode RGB palette from binary data with pre-allocation.
 * Helper to avoid code duplication and optimize memory allocation.
 */
function decodeRgbPalette(data: Uint8Array, offset: number, count: number): Array<RgbColor> {
    const palette = new Array<RgbColor>(count);
    let idx = offset;
    for (let i = 0; i < count; i++) {
        palette[i] = {
            r: data[idx++],
            g: data[idx++],
            b: data[idx++],
        };
    }
    return palette;
}

/**
 * Decode sparse binary territory snapshot from Rust backend.
 * Uses parallel typed arrays instead of objects to eliminate GC pressure.
 *
 * @param data Binary data array (serialized from Vec<u8> in Rust)
 * @returns Parallel arrays of indices and owner IDs, or null if invalid
 */
function decodeTerritorySnapshot(data: number[] | Uint8Array): DecodedSnapshot | null {
    // Convert to Uint8Array if needed for faster access
    const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);

    if (bytes.length < 4) {
        console.error("Invalid territory snapshot: not enough data for count");
        return null;
    }

    // Use DataView for faster little-endian reads
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

    // Read count (4 bytes, little-endian u32)
    const count = view.getUint32(0, true);

    // Sanity check: reject unreasonably large counts (likely malformed data)
    const MAX_TILES = 10_000_000; // Adjust based on your max map size
    if (count > MAX_TILES) {
        console.error(`Invalid territory snapshot: count ${count} exceeds maximum ${MAX_TILES}`);
        return null;
    }

    const expectedSize = 4 + count * 6;
    if (bytes.length !== expectedSize) {
        console.error(`Invalid territory snapshot: expected ${expectedSize} bytes, got ${bytes.length}`);
        return null;
    }

    // Pre-allocate typed arrays (no objects, no GC pressure)
    const indices = new Uint32Array(count);
    const ownerIds = new Uint16Array(count);

    // Decode directly into typed arrays
    for (let i = 0; i < count; i++) {
        const offset = 4 + i * 6;
        indices[i] = view.getUint32(offset, true);
        ownerIds[i] = view.getUint16(offset + 4, true);
    }

    return { indices, ownerIds };
}

/**
 * Decode binary initialization data sent from Rust backend.
 * Format: [terrain_len:4][terrain_data][territory_len:4][territory_data][nation_palette_count:2][nation_palette_rgb:N*3]
 * Terrain data: [width:2][height:2][tile_ids:N][palette_count:2][palette_rgb:N*3]
 * Territory data: [count:4][tiles...] where tiles = [index:4][owner:2]
 */
export function decodeInitBinary(data: Uint8Array): DecodedInitBinary | null {
    console.log(`Decoding init binary: ${data.length} bytes`);

    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    let offset = 0;

    // Parse terrain length and extract terrain data
    if (offset + 4 > data.length) {
        console.error("Invalid init binary: not enough data for terrain length");
        return null;
    }
    const terrainLen = view.getUint32(offset, true);
    offset += 4;

    if (offset + terrainLen > data.length) {
        console.error(`Invalid init binary: terrain data truncated (expected ${terrainLen} bytes)`);
        return null;
    }
    const terrainStart = offset;
    offset += terrainLen;

    // Parse territory length and extract territory data
    if (offset + 4 > data.length) {
        console.error("Invalid init binary: not enough data for territory length");
        return null;
    }
    const territoryLen = view.getUint32(offset, true);
    offset += 4;

    if (offset + territoryLen > data.length) {
        console.error(`Invalid init binary: territory data truncated (expected ${territoryLen} bytes)`);
        return null;
    }
    const territoryData = data.subarray(offset, offset + territoryLen);
    offset += territoryLen;

    // Decode terrain data using main DataView (optimization: reuse view instead of creating new one)
    if (terrainLen < 4) {
        console.error("Invalid terrain data: not enough data for dimensions");
        return null;
    }

    const width = view.getUint16(terrainStart, true);
    const height = view.getUint16(terrainStart + 2, true);

    const tileDataLength = width * height;
    if (terrainLen < 4 + tileDataLength + 2) {
        console.error("Invalid terrain data: not enough data for tile IDs and palette count");
        return null;
    }

    const tileIds = data.subarray(terrainStart + 4, terrainStart + 4 + tileDataLength);

    const paletteStart = terrainStart + 4 + tileDataLength;
    const paletteCount = view.getUint16(paletteStart, true);

    if (terrainLen < 4 + tileDataLength + 2 + paletteCount * 3) {
        console.error("Invalid terrain data: not enough data for palette colors");
        return null;
    }

    const palette = decodeRgbPalette(data, paletteStart + 2, paletteCount);

    console.log(`Decoded terrain: ${width}x${height}, ${paletteCount} colors`);

    // Decode territory data using existing function
    const territory = decodeTerritorySnapshot(territoryData);
    if (!territory) {
        console.error("Failed to decode territory data from init binary");
        return null;
    }

    console.log(`Decoded territory: ${territory.indices.length} claimed tiles`);

    // Decode nation palette
    if (offset + 2 > data.length) {
        console.error("Invalid init binary: not enough data for nation palette count");
        return null;
    }

    const nationPaletteCount = view.getUint16(offset, true);
    offset += 2;

    if (offset + nationPaletteCount * 3 > data.length) {
        console.error(`Invalid init binary: nation palette truncated (expected ${nationPaletteCount * 3} bytes)`);
        return null;
    }

    const nationPalette = decodeRgbPalette(data, offset, nationPaletteCount);

    console.log(`Decoded nation palette: ${nationPaletteCount} colors`);

    return {
        terrain: { width, height, tileIds, palette },
        territory,
        nationPalette,
    };
}
