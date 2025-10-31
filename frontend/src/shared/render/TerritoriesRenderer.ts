import { Container, Sprite, Texture, BufferImageSource, Filter, GlProgram } from "pixi.js";
import { RgbColor } from "@/shared/render";

// Territory ownership sentinel values (must match backend)
const OWNER_UNCLAIMED = 65535; // Unclaimed/water tiles (rendered as transparent)

/**
 * Territory rendering shaders
 *
 * This shader system renders nation ownership on the map using:
 * - RG8 texture storing u16 nation IDs (R=low byte, G=high byte)
 * - 256x256 palette texture for nation colors (supports up to 65536 nations)
 * - Edge detection for borders between different nations
 * - Highlighting system for UI hover states
 */

// Vertex shader - PixiJS v8 standard filter vertex shader (GLSL 100 ES)
const TERRITORY_VERTEX_SHADER = `
attribute vec2 aPosition;

varying vec2 vTextureCoord;
varying vec2 vFilterCoord;

uniform vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;

vec4 filterVertexPosition() {
    vec2 position = aPosition * uOutputFrame.zw + uOutputFrame.xy;
    position.x = position.x * (2.0 / uOutputTexture.x) - 1.0;
    position.y = position.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
    return vec4(position, 0.0, 1.0);
}

vec2 filterTextureCoord() {
    return aPosition * (uOutputFrame.zw * uInputSize.zw);
}

void main() {
    gl_Position = filterVertexPosition();
    vTextureCoord = filterTextureCoord();
    vFilterCoord = vTextureCoord;
}
`;

const TERRITORY_FRAGMENT_SHADER = `
precision mediump float;

varying vec2 vTextureCoord;

uniform sampler2D uTexture;
uniform vec2 uMapSize;
uniform sampler2D uPalette;
uniform float uHighlightedNation;

// Decode u16 from RG8 (R=low byte, G=high byte)
float decodeU16(vec2 rg) {
    return rg.r * 255.0 + rg.g * 65280.0;
}

void main() {
    vec4 center = texture2D(uTexture, vTextureCoord);
    float centerOwner = decodeU16(center.rg);

    // Unclaimed = 65535 (backend sends this for tiles with no owner)
    if (centerOwner >= 65535.0) {
        discard;
    }

    vec2 texelSize = 1.0 / uMapSize;

    bool isBorder = false;
    for (float dist = 1.0; dist <= 2.0; dist += 1.0) {
        vec2 offset = texelSize * dist;

        float left = decodeU16(texture2D(uTexture, vTextureCoord + vec2(-offset.x, 0.0)).rg);
        float right = decodeU16(texture2D(uTexture, vTextureCoord + vec2(offset.x, 0.0)).rg);
        float top = decodeU16(texture2D(uTexture, vTextureCoord + vec2(0.0, -offset.y)).rg);
        float bottom = decodeU16(texture2D(uTexture, vTextureCoord + vec2(0.0, offset.y)).rg);

        if (centerOwner != left || centerOwner != right ||
            centerOwner != top || centerOwner != bottom) {
            isBorder = true;
            break;
        }
    }

    // Map nation ID to 2D palette texture (256x256 grid = 65536 colors)
    // X = nation_id % 256, Y = nation_id / 256
    float x = mod(centerOwner, 256.0) / 256.0;
    float y = floor(centerOwner / 256.0) / 256.0;
    vec4 territoryColor = texture2D(uPalette, vec2(x, y));

    // Highlighting logic
    bool isHighlighting = uHighlightedNation >= 0.0;
    bool isHighlighted = (isHighlighting && centerOwner == uHighlightedNation);

    float alpha = isBorder ? 0.85 : 0.60;

    if (isBorder) {
        if (isHighlighted) {
            territoryColor.rgb = vec3(255, 255, 255);
        } else {
            territoryColor.rgb *= 0.85;
        }
    }

    gl_FragColor = vec4(territoryColor.rgb, alpha);
}
`;

export class TerritoryLayer {
    public readonly container: Container;

    private readonly sprite: Sprite;
    private readonly texture: Texture;
    private readonly bufferSource: BufferImageSource;

    private readonly ownerData: Uint16Array;
    private readonly textureData: Uint8Array;

    private readonly paletteTexture: Texture;
    private readonly paletteSource: BufferImageSource;
    private readonly paletteData: Uint8Array;

    private readonly territoryFilter: Filter;

    // Dirty region tracking for partial updates
    private isDirty: boolean = false;
    private dirtyMinX: number = 0;
    private dirtyMinY: number = 0;
    private dirtyMaxX: number = 0;
    private dirtyMaxY: number = 0;

    // Batching state
    private pendingChanges: Array<{ index: number; owner_id: number }> = [];
    private updateScheduled: boolean = false;

    private readonly width: number;
    private readonly height: number;

    constructor(width: number, height: number, palette: Array<RgbColor>) {
        this.width = width;
        this.height = height;

        this.container = new Container();

        // Initialize all tiles to unclaimed (65535)
        this.ownerData = new Uint16Array(width * height);
        this.ownerData.fill(65535);

        this.textureData = new Uint8Array(width * height * 2);
        // Support up to 65536 players (u16::MAX + 1)
        this.paletteData = new Uint8Array(65536 * 4);

        // Initialize palette
        for (let i = 0; i < Math.min(palette.length, 65536); i++) {
            const color = palette[i];
            const pixelIndex = i * 4;
            this.paletteData[pixelIndex] = color.r;
            this.paletteData[pixelIndex + 1] = color.g;
            this.paletteData[pixelIndex + 2] = color.b;
            this.paletteData[pixelIndex + 3] = 255;
        }

        // Use 256x256 2D texture (65536 total colors) to avoid WebGL texture size limits
        this.paletteSource = new BufferImageSource({
            resource: this.paletteData,
            width: 256,
            height: 256,
        });
        this.paletteTexture = new Texture({ source: this.paletteSource });

        const { sprite, texture, bufferSource } = this.initTexture();
        this.sprite = sprite;
        this.texture = texture;
        this.bufferSource = bufferSource;

        this.territoryFilter = this.setupShader();
    }

    private initTexture(): { sprite: Sprite; texture: Texture; bufferSource: BufferImageSource } {
        // RG8 format: R=low byte, G=high byte (stores u16 nation IDs)
        // Initialize all tiles to OWNER_UNCLAIMED which the shader will discard
        for (let i = 0; i < this.textureData.length; i += 2) {
            this.textureData[i] = OWNER_UNCLAIMED & 0xff; // R = 255
            this.textureData[i + 1] = (OWNER_UNCLAIMED >> 8) & 0xff; // G = 255
        }

        const bufferSource = new BufferImageSource({
            resource: this.textureData,
            width: this.width,
            height: this.height,
            format: "rg8unorm",
        });

        const texture = new Texture({ source: bufferSource });
        texture.source.scaleMode = "nearest";
        texture.source.update();

        const sprite = new Sprite(texture);
        sprite.scale.set(this.width / texture.width, this.height / texture.height);
        sprite.anchor.set(0.5, 0.5);
        sprite.x = 0;
        sprite.y = 0;
        sprite.alpha = 1.0;
        sprite.visible = true;
        sprite.renderable = true;
        sprite.cullable = false;
        sprite.blendMode = "screen";

        this.container.addChild(sprite);

        return { sprite, texture, bufferSource };
    }

    applySnapshot(snapshot: { indices: Uint32Array; ownerIds: Uint16Array }) {
        // Clear all tiles to unclaimed
        // The sparse format only includes player-owned tiles (0-65533)
        // All other tiles default to OWNER_UNCLAIMED which the shader discards
        this.ownerData.fill(OWNER_UNCLAIMED);

        // Fill texture data with OWNER_UNCLAIMED encoded as RG8 (low byte, high byte)
        for (let i = 0; i < this.textureData.length; i += 2) {
            this.textureData[i] = OWNER_UNCLAIMED & 0xff; // R = 255
            this.textureData[i + 1] = (OWNER_UNCLAIMED >> 8) & 0xff; // G = 255
        }

        // Apply claimed tiles from sparse snapshot using parallel arrays (no object allocations)
        const { indices, ownerIds } = snapshot;
        const length = indices.length;

        for (let i = 0; i < length; i++) {
            const index = indices[i];
            const owner_id = ownerIds[i];

            if (index < this.ownerData.length) {
                this.ownerData[index] = owner_id;

                const texIndex = index * 2;
                this.textureData[texIndex] = owner_id & 0xff;
                this.textureData[texIndex + 1] = (owner_id >> 8) & 0xff;
            }
        }

        this.isDirty = true;
        this.dirtyMinX = 0;
        this.dirtyMinY = 0;
        this.dirtyMaxX = this.width - 1;
        this.dirtyMaxY = this.height - 1;

        this.updateTexture();
    }

    applyDelta(changes: Array<{ index: number; owner_id: number }>) {
        if (changes.length === 0) {
            return;
        }

        this.pendingChanges.push(...changes);

        if (!this.updateScheduled) {
            this.updateScheduled = true;
            requestAnimationFrame(() => this.processPendingChanges());
        }
    }

    private processPendingChanges() {
        this.updateScheduled = false;

        if (this.pendingChanges.length === 0) {
            return;
        }

        let minX = this.width;
        let minY = this.height;
        let maxX = 0;
        let maxY = 0;

        for (const change of this.pendingChanges) {
            if (change.index >= 0 && change.index < this.ownerData.length) {
                const value = change.owner_id;

                // Update texture with ALL values, including 65535 (unclaimed)
                // The shader will discard unclaimed tiles during rendering
                this.ownerData[change.index] = value;

                const texIndex = change.index * 2;
                this.textureData[texIndex] = value & 0xff;
                this.textureData[texIndex + 1] = (value >> 8) & 0xff;

                const x = change.index % this.width;
                const y = Math.floor(change.index / this.width);
                minX = Math.min(minX, x);
                minY = Math.min(minY, y);
                maxX = Math.max(maxX, x);
                maxY = Math.max(maxY, y);
            }
        }

        this.pendingChanges = [];

        if (maxX >= minX && maxY >= minY) {
            this.isDirty = true;
            this.dirtyMinX = minX;
            this.dirtyMinY = minY;
            this.dirtyMaxX = maxX;
            this.dirtyMaxY = maxY;
            this.updateTexture();
        }
    }

    private updateTexture() {
        if (!this.isDirty) {
            return;
        }

        const imageData = this.bufferSource.resource as Uint8Array;
        const gl = (this.texture.source as any)._glTextures?.[0]?.gl;
        const glTexture = (this.texture.source as any)._glTextures?.[0]?.texture;

        // If GL context not ready, fall back to full upload
        if (!gl || !glTexture) {
            this.bufferSource.update();
            this.isDirty = false;
            return;
        }

        // Calculate dirty region dimensions
        const regionWidth = this.dirtyMaxX - this.dirtyMinX + 1;
        const regionHeight = this.dirtyMaxY - this.dirtyMinY + 1;
        const totalPixels = this.width * this.height;
        const dirtyPixels = regionWidth * regionHeight;

        // Use partial update if dirty region is small (< 25% of texture)
        if (dirtyPixels < totalPixels * 0.25) {
            // Extract dirty region into contiguous buffer (R8 format)
            const regionData = new Uint8Array(regionWidth * regionHeight);

            for (let y = 0; y < regionHeight; y++) {
                const srcY = this.dirtyMinY + y;
                const srcOffset = srcY * this.width + this.dirtyMinX;
                const dstOffset = y * regionWidth;

                // Copy one row at a time (single channel)
                regionData.set(imageData.subarray(srcOffset, srcOffset + regionWidth), dstOffset);
            }

            // Upload dirty region only
            gl.bindTexture(gl.TEXTURE_2D, glTexture);
            gl.texSubImage2D(
                gl.TEXTURE_2D,
                0, // mip level
                this.dirtyMinX,
                this.dirtyMinY,
                regionWidth,
                regionHeight,
                gl.RED,
                gl.UNSIGNED_BYTE,
                regionData,
            );
        } else {
            // Dirty region is large, upload full texture
            this.bufferSource.update();
        }

        // Clear dirty flag
        this.isDirty = false;
    }

    private setupShader(): Filter {
        // Create filter with the territory shader
        const filter = new Filter({
            glProgram: new GlProgram({
                vertex: TERRITORY_VERTEX_SHADER,
                fragment: TERRITORY_FRAGMENT_SHADER,
            }),
            resources: {
                // uTexture is auto-bound to sprite texture
                uPalette: this.paletteTexture.source,
                territoryUniforms: {
                    uMapSize: { value: [this.width, this.height], type: "vec2<f32>" },
                    uHighlightedNation: { value: -1, type: "f32" },
                },
            },
        });

        // Apply filter to sprite
        this.sprite.filters = [filter];

        return filter;
    }

    // Get owner at specific tile
    getOwnerAt(tileX: number, tileY: number): number {
        if (tileX >= 0 && tileX < this.width && tileY >= 0 && tileY < this.height) {
            const index = tileY * this.width + tileX;
            return this.ownerData[index];
        }
        return 0;
    }

    // Set highlighted nation (-1 or null to clear)
    setHighlightedNation(nationId: number | null) {
        this.territoryFilter.resources.territoryUniforms.uniforms.uHighlightedNation = nationId ?? -1;
    }

    // Clear all territory
    clear() {
        this.ownerData.fill(OWNER_UNCLAIMED);
        const imageData = this.bufferSource.resource as Uint8Array;

        // Clear all pixels to OWNER_UNCLAIMED
        for (let i = 0; i < imageData.length; i += 2) {
            imageData[i] = OWNER_UNCLAIMED & 0xff;
            imageData[i + 1] = (OWNER_UNCLAIMED >> 8) & 0xff;
        }

        // Mark entire texture as dirty
        this.isDirty = true;
        this.dirtyMinX = 0;
        this.dirtyMinY = 0;
        this.dirtyMaxX = this.width - 1;
        this.dirtyMaxY = this.height - 1;

        this.updateTexture();
    }
}
