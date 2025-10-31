/**
 * Utility for converting between tile indices, tile coordinates, and world positions.
 *
 * Coordinate systems:
 * - Tile index: Linear index into terrain array (0 to width*height-1)
 * - Tile coords: (x, y) grid coordinates (0,0 = top-left corner)
 * - World position: Centered coordinates where (0,0) = map center, used for rendering
 */
export class CoordinateMapper {
    private mapWidth: number;
    private mapHeight: number;

    constructor(mapWidth: number, mapHeight: number) {
        this.mapWidth = mapWidth;
        this.mapHeight = mapHeight;
    }

    /**
     * Convert tile index to tile coordinates (x, y)
     */
    tileIndexToCoords(tileIndex: number): { x: number; y: number } {
        return {
            x: tileIndex % this.mapWidth,
            y: Math.floor(tileIndex / this.mapWidth),
        };
    }

    /**
     * Convert tile coordinates (x, y) to tile index
     */
    coordsToTileIndex(tileX: number, tileY: number): number {
        return tileY * this.mapWidth + tileX;
    }

    /**
     * Convert tile index to world position (centered at tile center)
     */
    tileIndexToWorld(tileIndex: number): { x: number; y: number } {
        const { x, y } = this.tileIndexToCoords(tileIndex);
        return this.tileCoordsToWorld(x, y);
    }

    /**
     * Convert tile coordinates to world position (centered at tile center)
     */
    tileCoordsToWorld(tileX: number, tileY: number): { x: number; y: number } {
        const worldX = tileX + 0.5 - this.mapWidth / 2;
        const worldY = tileY + 0.5 - this.mapHeight / 2;
        return { x: worldX, y: worldY };
    }

    /**
     * Convert world position to tile coordinates (returns null if out of bounds)
     */
    worldToTileCoords(worldX: number, worldY: number): { x: number; y: number } | null {
        const adjustedX = worldX + this.mapWidth / 2;
        const adjustedY = worldY + this.mapHeight / 2;

        const tileX = Math.floor(adjustedX);
        const tileY = Math.floor(adjustedY);

        if (tileX >= 0 && tileX < this.mapWidth && tileY >= 0 && tileY < this.mapHeight) {
            return { x: tileX, y: tileY };
        }

        return null;
    }

    /**
     * Convert world position to tile index (returns null if out of bounds)
     */
    worldToTileIndex(worldX: number, worldY: number): number | null {
        const coords = this.worldToTileCoords(worldX, worldY);
        return coords !== null ? this.coordsToTileIndex(coords.x, coords.y) : null;
    }

    /**
     * Check if tile coordinates are within map bounds
     */
    isInBounds(tileX: number, tileY: number): boolean {
        return tileX >= 0 && tileX < this.mapWidth && tileY >= 0 && tileY < this.mapHeight;
    }

    /**
     * Check if tile index is within map bounds
     */
    isValidTileIndex(tileIndex: number): boolean {
        return tileIndex >= 0 && tileIndex < this.mapWidth * this.mapHeight;
    }

    /**
     * Get map dimensions in world units
     */
    getMapWorldSize(): { width: number; height: number } {
        return {
            width: this.mapWidth,
            height: this.mapHeight,
        };
    }
}
