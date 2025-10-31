import { RgbColor } from "@/shared/render";
import { Sprite, Texture, BufferImageSource } from "pixi.js";

export interface TerrainTextureResult {
    texture: Texture;
    sprite: Sprite;
}

/**
 * Builds terrain textures from tile type data and color palettes.
 * Uses optimized Uint32 writes for fast texture generation.
 */
export class TerrainTextureBuilder {
    /**
     * Create a terrain texture from tile type IDs and a color palette
     */
    static createTerrainTexture(
        terrainData: Uint8Array | number[],
        palette: RgbColor[],
        width: number,
        height: number,
    ): TerrainTextureResult {
        // Normalize terrain_data to Uint8Array for consistent handling
        const normalizedTerrainData = terrainData instanceof Uint8Array ? terrainData : new Uint8Array(terrainData);

        // Create texture from tile type IDs
        const totalPixels = width * height;
        const imageData = new Uint8Array(totalPixels * 4); // RGBA

        // Pre-compute RGBA tuples for all palette entries (cache-friendly)
        const paletteRGBA = new Uint32Array(palette.length);
        for (let i = 0; i < palette.length; i++) {
            const c = palette[i];
            // Pack RGBA as single uint32 (little-endian: ABGR in memory)
            paletteRGBA[i] = (255 << 24) | (c.b << 16) | (c.g << 8) | c.r;
        }

        // Fast path: Use Uint32 view for 4x fewer writes
        const imageData32 = new Uint32Array(imageData.buffer);
        const defaultColor = (255 << 24) | (100 << 16) | (100 << 8) | 100; // Default gray

        for (let i = 0; i < totalPixels; i++) {
            const tileTypeId = normalizedTerrainData[i];
            imageData32[i] = paletteRGBA[tileTypeId] ?? defaultColor;
        }

        const bufferSource = new BufferImageSource({
            resource: imageData,
            width,
            height,
        });

        // Create the texture
        const texture = new Texture({ source: bufferSource });

        // Use nearest neighbor filtering for crisp pixel edges
        texture.source.scaleMode = "nearest";

        // Mark source as needing update
        bufferSource.update();

        // Create sprite with the texture
        const sprite = new Sprite(texture);

        // Set dimensions explicitly (don't use width/height setters which can cause issues)
        sprite.scale.set(width / texture.width, height / texture.height);

        // Center the terrain
        sprite.anchor.set(0.5, 0.5);
        sprite.x = 0;
        sprite.y = 0;

        // Ensure sprite is visible and rendered
        sprite.alpha = 1.0;
        sprite.visible = true;
        sprite.renderable = true;

        // Disable culling to prevent disappearing at certain zoom levels
        sprite.cullable = false;

        return { texture, sprite };
    }
}
