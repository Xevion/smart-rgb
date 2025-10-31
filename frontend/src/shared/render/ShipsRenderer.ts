import { Container, Graphics } from "pixi.js";
import { CoordinateMapper } from "@/shared/render/CoordinateMapper";
import type { ShipUpdateVariant } from "@/shared/api/types";
import { RgbColor } from "@/shared/render";

// Must match backend SHIP_TICKS_PER_TILE constant
const SHIP_TICKS_PER_TILE = 1;

// Ship visual size constants
const MIN_SHIP_SIZE = 0.15; // Minimum radius for ships with few troops
const MAX_SHIP_SIZE = 0.5; // Maximum radius for large troop counts
const SHIP_SIZE_SCALE_FACTOR = 1001; // Logarithmic scale factor (1 troop = MIN, 1000 troops ≈ MAX)

interface ShipState {
    owner_nation_id: number;
    path: number[];
    current_path_index: number;
    ticks_until_move: number;
    troops: number;
    sprite: Container;
}

export class ShipLayer {
    public readonly container: Container;
    private readonly ships: Map<number, ShipState>;
    private readonly palette: Array<RgbColor>;
    private readonly coordinateMapper: CoordinateMapper;

    constructor(coordinateMapper: CoordinateMapper, palette: Array<RgbColor>) {
        this.container = new Container();
        this.container.zIndex = 2; // Above territory layer
        this.ships = new Map();
        this.palette = palette;
        this.coordinateMapper = coordinateMapper;
    }

    /**
     * Process ship update variants (Create/Move/Destroy)
     */
    processUpdates(updates: ShipUpdateVariant[]) {
        for (const update of updates) {
            switch (update.type) {
                case "Create":
                    this.createShip(update);
                    break;
                case "Move":
                    this.moveShip(update);
                    break;
                case "Destroy":
                    this.destroyShip(update);
                    break;
            }
        }
    }

    /**
     * Create a new ship with full state
     */
    private createShip(update: Extract<ShipUpdateVariant, { type: "Create" }>) {
        const sprite = this.createShipSprite(update.owner_nation_id, update.troops);

        const shipState: ShipState = {
            owner_nation_id: update.owner_nation_id,
            path: update.path,
            current_path_index: 0,
            ticks_until_move: SHIP_TICKS_PER_TILE,
            troops: update.troops,
            sprite,
        };

        this.ships.set(update.id, shipState);
        this.container.addChild(sprite);

        // Position at start of path
        this.updateShipPosition(shipState);
    }

    /**
     * Update ship to next tile in path
     */
    private moveShip(update: Extract<ShipUpdateVariant, { type: "Move" }>) {
        const ship = this.ships.get(update.id);
        if (!ship) return;

        ship.current_path_index = update.current_path_index;
        ship.ticks_until_move = SHIP_TICKS_PER_TILE;

        this.updateShipPosition(ship);
    }

    /**
     * Remove ship from map
     */
    private destroyShip(update: Extract<ShipUpdateVariant, { type: "Destroy" }>) {
        const ship = this.ships.get(update.id);
        if (!ship) return;

        this.container.removeChild(ship.sprite);
        ship.sprite.destroy({ children: true });
        this.ships.delete(update.id);
    }

    /**
     * Create a ship sprite (circle with logarithmic size scaling)
     */
    private createShipSprite(owner_nation_id: number, troops: number): Container {
        const container = new Container();

        const color: RgbColor = this.palette[owner_nation_id] || { r: 128, g: 128, b: 128 };
        const hexColor = (color.r << 16) | (color.g << 8) | color.b;

        const graphics = new Graphics();

        // Logarithmic size scaling based on troop count
        const size = MIN_SHIP_SIZE + (Math.log10(troops + 1) / Math.log10(SHIP_SIZE_SCALE_FACTOR)) * (MAX_SHIP_SIZE - MIN_SHIP_SIZE);

        // Draw circle
        graphics.circle(0, 0, size);
        graphics.fill({ color: hexColor, alpha: 0.9 });

        container.addChild(graphics);
        return container;
    }

    /**
     * Update ship position with interpolation
     */
    private updateShipPosition(ship: ShipState) {
        const currentTile = ship.path[ship.current_path_index];
        const nextTile = ship.path[ship.current_path_index + 1];

        // Get world position of current tile
        const currentWorld = this.coordinateMapper.tileIndexToWorld(currentTile);
        let worldX = currentWorld.x;
        let worldY = currentWorld.y;

        // Interpolate to next tile if available
        if (nextTile !== undefined) {
            const nextWorld = this.coordinateMapper.tileIndexToWorld(nextTile);

            // Calculate interpolation progress (0 to 1)
            const progress = 1.0 - ship.ticks_until_move / SHIP_TICKS_PER_TILE;

            // Lerp between current and next position
            worldX = worldX + (nextWorld.x - worldX) * progress;
            worldY = worldY + (nextWorld.y - worldY) * progress;
        }

        ship.sprite.x = worldX;
        ship.sprite.y = worldY;
    }

    /**
     * Update interpolation for all ships (called each render frame)
     */
    update(deltaTime: number) {
        for (const ship of this.ships.values()) {
            // Decrement ticks_until_move locally
            ship.ticks_until_move = Math.max(0, ship.ticks_until_move - deltaTime);

            // Update visual position
            this.updateShipPosition(ship);
        }
    }

    /**
     * Clear all ships
     */
    clear() {
        for (const ship of this.ships.values()) {
            this.container.removeChild(ship.sprite);
            ship.sprite.destroy({ children: true });
        }
        this.ships.clear();
    }

    /**
     * Destroy the layer and clean up resources
     */
    destroy() {
        this.clear();
        this.container.destroy();
    }
}
