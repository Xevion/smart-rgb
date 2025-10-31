import { Application, Container, WebGPURenderer, Renderer, WebGLRenderer } from "pixi.js";
import { CameraController } from "@/shared/render/CameraController";
import { TerritoryLayer } from "@/shared/render/TerritoriesRenderer";
import { ShipLayer } from "@/shared/render/ShipsRenderer";
import { CoordinateMapper } from "@/shared/render/CoordinateMapper";
import { TerrainTextureBuilder } from "@/shared/render/TerrainTextureBuilder";
import type { ShipsUpdatePayload } from "@/shared/api/types";
import { RgbColor } from "@/shared/render";

// Renderer configuration constants
const DEFAULT_BACKGROUND_COLOR = 0x1a1a2e; // Dark blue-grey
const MIN_CAMERA_SCALE = 0.5;
const MAX_CAMERA_SCALE = 13.5;
const FIT_ZOOM_PADDING = 0.8; // 0.8 = 20% padding around map when fitting to viewport
const GAME_TICKS_PER_SECOND = 10;
const TARGET_FPS = 60;

export interface TileChange {
    index: number;
    owner_id: number;
}

export interface TerrainData {
    size: { x: number; y: number }; // Normalized from glam::U16Vec2 (WASM converts [x, y] → { x, y })
    terrain_data: Uint8Array | number[]; // Tile type IDs (u8 values from backend)
}

export interface PaletteData {
    colors: Array<RgbColor>;
}

export interface GameRendererConfig {
    canvas: HTMLCanvasElement;
    terrainPalette: PaletteData;
    terrain: TerrainData;
    nationPalette: PaletteData;
    initialTerritories: {
        turn: number;
        territories: { indices: Uint32Array; ownerIds: Uint16Array };
    };
}

export class GameRenderer {
    private readonly app: Application;
    private readonly territoryLayer: TerritoryLayer;
    private readonly shipLayer: ShipLayer;
    private readonly cameraController: CameraController;
    public readonly coordinateMapper: CoordinateMapper;
    private readonly resizeHandler: () => void;

    private constructor(
        app: Application,
        territoryLayer: TerritoryLayer,
        shipLayer: ShipLayer,
        cameraController: CameraController,
        coordinateMapper: CoordinateMapper,
        resizeHandler: () => void,
    ) {
        this.app = app;
        this.territoryLayer = territoryLayer;
        this.shipLayer = shipLayer;
        this.cameraController = cameraController;
        this.coordinateMapper = coordinateMapper;
        this.resizeHandler = resizeHandler;
    }

    static async create(config: GameRendererConfig): Promise<GameRenderer> {
        const { canvas, terrainPalette, terrain, nationPalette, initialTerritories } = config;

        // Validate canvas dimensions
        const width = canvas.clientWidth || canvas.width || 800;
        const height = canvas.clientHeight || canvas.height || 600;

        if (width === 0 || height === 0) {
            throw new Error(
                `Canvas has invalid dimensions: ${width}x${height}. ` +
                    `Ensure the canvas element and its parent have explicit dimensions.`,
            );
        }

        // Initialize Pixi application
        const app = new Application();
        await app.init({
            canvas,
            width,
            height,
            backgroundColor: DEFAULT_BACKGROUND_COLOR,
            antialias: false,
            resolution: window.devicePixelRatio || 1,
            autoDensity: true,
        });

        // Verify initialization succeeded
        if (!app.stage) {
            throw new Error("PixiJS Application failed to initialize - stage is null");
        }

        // Create main container
        const mainContainer = new Container();
        app.stage.addChild(mainContainer);

        // Create layers container (this gets transformed by camera)
        const layersContainer = new Container();
        layersContainer.sortableChildren = true; // Enable z-index sorting
        mainContainer.addChild(layersContainer);

        // Extract terrain dimensions
        const mapWidth = terrain.size.x;
        const mapHeight = terrain.size.y;

        // Create coordinate mapper
        const coordinateMapper = new CoordinateMapper(mapWidth, mapHeight);

        // Layer z-index ordering: 0=terrain (bottom), 1=territories, 2=ships (top)

        // 1. Terrain layer (water/land/mountains)
        const terrainLayer = new Container();
        terrainLayer.zIndex = 0;
        layersContainer.addChild(terrainLayer);

        const { sprite: terrainSprite } = TerrainTextureBuilder.createTerrainTexture(
            terrain.terrain_data,
            terrainPalette.colors,
            mapWidth,
            mapHeight,
        );
        terrainLayer.addChild(terrainSprite);

        // 2. Territory layer (nation borders and ownership)
        const territoryLayer = new TerritoryLayer(mapWidth, mapHeight, nationPalette.colors);
        territoryLayer.container.zIndex = 1;
        layersContainer.addChild(territoryLayer.container);

        territoryLayer.applySnapshot(initialTerritories.territories);

        // 3. Ship layer (animated units moving between territories)
        const shipLayer = new ShipLayer(coordinateMapper, nationPalette.colors);
        shipLayer.container.zIndex = 2;
        layersContainer.addChild(shipLayer.container);

        // Initialize camera controller
        const cameraController = new CameraController(layersContainer, canvas);

        // Handle window resize
        const resizeHandler = () => {
            const parent = canvas.parentElement;
            if (parent) {
                const resizeWidth = parent.clientWidth;
                const resizeHeight = parent.clientHeight;

                app.renderer.resize(resizeWidth, resizeHeight);
                canvas.style.width = `${resizeWidth}px`;
                canvas.style.height = `${resizeHeight}px`;
            }
        };
        window.addEventListener("resize", resizeHandler);

        // Add ticker for ship interpolation updates
        app.ticker.add((ticker) => {
            // Convert frame deltaTime to game ticks
            // deltaTime is in frames (60fps = 1.0), convert to game ticks per frame
            const tickDelta = ticker.deltaTime * (GAME_TICKS_PER_SECOND / TARGET_FPS);
            shipLayer.update(tickDelta);
        });

        // Force an immediate render to ensure texture is processed
        app.renderer.render(app.stage);

        // Center camera on map and set initial zoom
        // Calculate zoom to fit map in viewport with padding
        const fitZoom = Math.min(width / mapWidth, height / mapHeight) * FIT_ZOOM_PADDING;

        // Clamp zoom to valid range
        const initialZoom = Math.max(MIN_CAMERA_SCALE, Math.min(MAX_CAMERA_SCALE, fitZoom));

        // Apply zoom first
        cameraController.setZoom(initialZoom, false);

        // Center on world origin (0, 0) since terrain sprite is already centered there
        cameraController.panBy(width / 2, height / 2, false);

        // Create and return the renderer instance
        return new GameRenderer(app, territoryLayer, shipLayer, cameraController, coordinateMapper, resizeHandler);
    }

    // Update ships with new variant-based system
    updateShips(data: ShipsUpdatePayload) {
        this.shipLayer.processUpdates(data.updates);
    }

    // Handle binary territory delta (used by both WASM and Tauri)
    updateBinaryDelta(data: Uint8Array) {
        // Decode binary format: [turn:8][count:4][changes...]
        if (data.length < 12) {
            return;
        }

        const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
        // Turn number at offset 0-7, not currently used
        const count = view.getUint32(8, true);

        const expectedSize = 12 + count * 6;
        if (data.length !== expectedSize) {
            console.error(`Invalid binary delta size: expected ${expectedSize}, got ${data.length}`);
            return;
        }

        const changes: TileChange[] = [];
        for (let i = 0; i < count; i++) {
            const offset = 12 + i * 6;
            const index = view.getUint32(offset, true);
            const owner_id = view.getUint16(offset + 4, true);
            changes.push({ index, owner_id });
        }

        this.territoryLayer.applyDelta(changes);
    }

    // Check if the last interaction was a camera drag
    hadCameraInteraction(): boolean {
        return this.cameraController.hadCameraInteraction();
    }

    // Convert screen position to world coordinates
    screenToWorld(screenX: number, screenY: number): { x: number; y: number } {
        return this.cameraController.screenToWorld(screenX, screenY);
    }

    // Convert screen position to tile index
    screenToTile(screenX: number, screenY: number): number | null {
        const worldPos = this.cameraController.screenToWorld(screenX, screenY);
        return this.coordinateMapper.worldToTileIndex(worldPos.x, worldPos.y);
    }

    // Get nation ID at a tile index
    getNationAtTile(tileIndex: number | null): number | null {
        if (tileIndex === null) {
            return null;
        }

        const { x: tileX, y: tileY } = this.coordinateMapper.tileIndexToCoords(tileIndex);
        const nationId = this.territoryLayer.getOwnerAt(tileX, tileY);

        // Nation IDs: 0-65533 (valid), 65534 (unclaimed), 65535 (water)
        return nationId < 65534 ? nationId : null;
    }

    // Set highlighted nation
    setHighlightedNation(nationId: number | null) {
        this.territoryLayer.setHighlightedNation(nationId);
    }

    // Get renderer information for analytics
    getRendererInfo(): {
        renderer: string;
        gpu_vendor?: string;
        gpu_device?: string;
    } {
        if (!this.app || !this.app.renderer) {
            return {
                renderer: "unknown",
            };
        }

        const renderer = this.app.renderer;

        const isWebGLRenderer = (renderer: Renderer): renderer is WebGLRenderer => {
            return (renderer as any).gl != undefined;
        };

        const isWebGPURenderer = (renderer: Renderer): renderer is WebGPURenderer => {
            return (renderer as any).gpu != undefined;
        };

        let gpuVendor: string | undefined;
        let gpuDevice: string | undefined;
        let rendererName = "unknown";

        // Try to extract GPU info from WebGPU adapter
        if (isWebGPURenderer(renderer)) {
            const gpuAdapter = renderer.gpu?.adapter;
            gpuVendor = gpuAdapter.info?.vendor;
            gpuDevice = gpuAdapter.info?.device;
            rendererName = "webgpu";
        }
        // Fallback to WebGL renderer info
        else if (isWebGLRenderer(renderer)) {
            const gl = (renderer as WebGLRenderer).gl;
            const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
            if (debugInfo) {
                gpuVendor = gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL);
                gpuDevice = gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
            }
            rendererName = "webgl";
        }

        return {
            renderer: rendererName,
            gpu_vendor: gpuVendor,
            gpu_device: gpuDevice,
        };
    }

    // Clean up
    destroy() {
        // Remove window resize listener
        window.removeEventListener("resize", this.resizeHandler);

        // Destroy camera controller
        this.cameraController.destroy();

        // Destroy layers
        this.territoryLayer.container.destroy({ children: true });
        this.shipLayer.destroy();

        // Destroy PixiJS application (this will clean up all containers and textures)
        this.app.destroy(true, { children: true, texture: true });
    }
}
