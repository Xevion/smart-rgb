import { Container } from "pixi.js";

// Camera zoom constraints (shared with GameRenderer)
const MIN_CAMERA_SCALE = 0.5;
const MAX_CAMERA_SCALE = 13.5;

export class CameraController {
    private container: Container;
    private canvas: HTMLCanvasElement;
    private isDragging = false;
    private lastMouseX = 0;
    private lastMouseY = 0;
    private mouseDownX = 0;
    private mouseDownY = 0;
    private hasDragged = false;
    private scale = 1;
    private readonly minScale = MIN_CAMERA_SCALE;
    private readonly maxScale = MAX_CAMERA_SCALE;

    // Zoom sensitivity configuration
    private readonly BASE_ZOOM_SENSITIVITY = 0.15; // Base: 10% zoom per scroll
    private readonly ZOOM_IN_SENSITIVITY = 1.1; // 90% of base (10% less sensitive)
    private readonly ZOOM_OUT_SENSITIVITY = 0.9; // 120% of base (20% more sensitive)

    // Smooth interpolation state
    private targetX = 0;
    private targetY = 0;
    private targetScale = 1;
    private lerpFactor = 0.15; // Smoothness factor (0.1-0.2 is good range)
    private isAnimating = false;

    // Animation loop control
    private animationFrameId: number | null = null;

    // Event listener references for cleanup
    private wheelHandler?: (e: WheelEvent) => void;
    private mouseDownHandler?: (e: MouseEvent) => void;
    private mouseMoveHandler?: (e: MouseEvent) => void;
    private mouseUpHandler?: () => void;

    // Track if window listeners are attached (during drag)
    private windowListenersAttached = false;

    constructor(container: Container, canvas: HTMLCanvasElement) {
        this.container = container;
        this.canvas = canvas;
        this.setupEventListeners(canvas);
        this.startAnimationLoop();
    }

    private startAnimationLoop() {
        // Animation loop is started on demand, not continuously
        // This is handled by updateSmooth() which requests next frame if needed
    }

    private lerp(current: number, target: number, factor: number): number {
        return current + (target - current) * factor;
    }

    private updateSmooth() {
        const threshold = 0.01; // Stop animating when close enough

        // Interpolate position
        const oldX = this.container.x;
        const oldY = this.container.y;
        this.container.x = this.lerp(this.container.x, this.targetX, this.lerpFactor);
        this.container.y = this.lerp(this.container.y, this.targetY, this.lerpFactor);

        // Interpolate scale
        const oldScale = this.scale;
        this.scale = this.lerp(this.scale, this.targetScale, this.lerpFactor);
        this.container.scale.set(this.scale);

        // Check if we're close enough to stop animating
        const posChanged = Math.abs(this.container.x - oldX) > threshold || Math.abs(this.container.y - oldY) > threshold;
        const scaleChanged = Math.abs(this.scale - oldScale) > threshold;

        if (posChanged || scaleChanged) {
            if (!this.isAnimating) {
                this.isAnimating = true;
            }
            // Continue animation
            this.animationFrameId = requestAnimationFrame(() => this.updateSmooth());
        } else if (this.isAnimating) {
            this.isAnimating = false;
            // Snap to final position
            this.container.x = this.targetX;
            this.container.y = this.targetY;
            this.scale = this.targetScale;
            this.container.scale.set(this.scale);
            // Animation stopped - no need to request next frame
            this.animationFrameId = null;
        }
    }

    private ensureAnimationRunning() {
        // Start animation loop if not already running
        if (this.animationFrameId === null) {
            this.animationFrameId = requestAnimationFrame(() => this.updateSmooth());
        }
    }

    private setupEventListeners(canvas: HTMLCanvasElement) {
        // Mouse wheel zoom
        this.wheelHandler = (e: WheelEvent) => {
            // Skip zoom when shift is pressed (reserved for attack controls)
            if (e.shiftKey) {
                return;
            }

            e.preventDefault();

            const delta = -e.deltaY;
            const scaleFactor =
                delta > 0
                    ? 1 + this.BASE_ZOOM_SENSITIVITY * this.ZOOM_IN_SENSITIVITY // Zooming in: base * 0.9
                    : 1 - this.BASE_ZOOM_SENSITIVITY * this.ZOOM_OUT_SENSITIVITY; // Zooming out: base * 1.2
            const newScale = this.targetScale * scaleFactor;

            if (newScale >= this.minScale && newScale <= this.maxScale) {
                // Get mouse position relative to canvas
                const rect = canvas.getBoundingClientRect();
                const mouseX = e.clientX - rect.left;
                const mouseY = e.clientY - rect.top;

                // Calculate world position before zoom using TARGET scale
                const worldX = (mouseX - this.targetX) / this.targetScale;
                const worldY = (mouseY - this.targetY) / this.targetScale;

                // Update target scale
                this.targetScale = newScale;

                // Adjust target position to keep mouse point stable
                this.targetX = mouseX - worldX * this.targetScale;
                this.targetY = mouseY - worldY * this.targetScale;

                // Ensure animation loop is running
                this.ensureAnimationRunning();
            }
        };

        // Mouse down - prepare for potential drag
        this.mouseDownHandler = (e: MouseEvent) => {
            if (e.button === 0) {
                // Left mouse button only
                this.isDragging = true;
                this.hasDragged = false;
                this.lastMouseX = e.clientX;
                this.lastMouseY = e.clientY;
                this.mouseDownX = e.clientX;
                this.mouseDownY = e.clientY;

                // Attach window-level listeners to track mouse even over UI elements
                this.attachWindowListeners();
            }
        };

        // Mouse move - pan
        this.mouseMoveHandler = (e: MouseEvent) => {
            if (this.isDragging) {
                const dx = e.clientX - this.lastMouseX;
                const dy = e.clientY - this.lastMouseY;

                // Check if user has moved enough to count as a drag (3px threshold)
                const totalMoved = Math.abs(e.clientX - this.mouseDownX) + Math.abs(e.clientY - this.mouseDownY);
                if (totalMoved > 3 && !this.hasDragged) {
                    this.hasDragged = true;
                    canvas.style.cursor = "grabbing";
                }

                // Update target position for smooth dragging
                this.targetX += dx;
                this.targetY += dy;

                // Also update current position directly for responsive feel during drag
                this.container.x += dx;
                this.container.y += dy;

                this.lastMouseX = e.clientX;
                this.lastMouseY = e.clientY;
            }
        };

        // Mouse up - stop dragging
        const stopDragging = () => {
            if (this.isDragging) {
                this.isDragging = false;
                canvas.style.cursor = "default";

                // Remove window-level listeners
                this.detachWindowListeners();
            }
        };

        this.mouseUpHandler = stopDragging;

        canvas.addEventListener("wheel", this.wheelHandler);
        canvas.addEventListener("mousedown", this.mouseDownHandler);
    }

    // Attach mousemove and mouseup to window during drag
    private attachWindowListeners() {
        if (this.windowListenersAttached) return;

        if (this.mouseMoveHandler) {
            window.addEventListener("mousemove", this.mouseMoveHandler);
        }
        if (this.mouseUpHandler) {
            window.addEventListener("mouseup", this.mouseUpHandler);
        }

        this.windowListenersAttached = true;
    }

    // Detach window listeners when drag ends
    private detachWindowListeners() {
        if (!this.windowListenersAttached) return;

        if (this.mouseMoveHandler) {
            window.removeEventListener("mousemove", this.mouseMoveHandler);
        }
        if (this.mouseUpHandler) {
            window.removeEventListener("mouseup", this.mouseUpHandler);
        }

        this.windowListenersAttached = false;
    }

    // Check if the last interaction was a drag (for click filtering)
    public hadCameraInteraction(): boolean {
        return this.hasDragged;
    }

    // Public methods for backend control
    centerOnTile(tileX: number, tileY: number, animate: boolean = false) {
        // Convert tile coordinates to world position (center of tile)
        const worldX = tileX + 0.5;
        const worldY = tileY + 0.5;

        // Center the camera on the world position
        const newX = this.canvas.width / 2 - worldX * this.scale;
        const newY = this.canvas.height / 2 - worldY * this.scale;

        if (animate) {
            // Smooth animation - set target
            this.targetX = newX;
            this.targetY = newY;
            this.ensureAnimationRunning();
        } else {
            // Instant - set both current and target
            this.container.x = newX;
            this.container.y = newY;
            this.targetX = newX;
            this.targetY = newY;
        }
    }

    setZoom(zoom: number, animate: boolean = false) {
        const newScale = Math.max(this.minScale, Math.min(this.maxScale, zoom));

        if (animate) {
            // Smooth animation - set target
            this.targetScale = newScale;
            this.ensureAnimationRunning();
        } else {
            // Instant - set both current and target
            this.scale = newScale;
            this.targetScale = newScale;
            this.container.scale.set(this.scale);
        }
    }

    panBy(dx: number, dy: number, animate: boolean = false) {
        if (animate) {
            // Smooth animation - add to target
            this.targetX += dx;
            this.targetY += dy;
            this.ensureAnimationRunning();
        } else {
            // Instant - add to both current and target
            this.container.x += dx;
            this.container.y += dy;
            this.targetX += dx;
            this.targetY += dy;
        }
    }

    reset() {
        this.scale = 1;
        this.targetScale = 1;
        this.container.scale.set(1);
        this.container.x = 0;
        this.container.y = 0;
        this.targetX = 0;
        this.targetY = 0;
    }

    // Convert screen coordinates to world coordinates
    screenToWorld(screenX: number, screenY: number): { x: number; y: number } {
        const worldX = (screenX - this.container.x) / this.scale;
        const worldY = (screenY - this.container.y) / this.scale;
        return { x: worldX, y: worldY };
    }

    // Convert world coordinates to screen coordinates
    worldToScreen(worldX: number, worldY: number): { x: number; y: number } {
        const screenX = worldX * this.scale + this.container.x;
        const screenY = worldY * this.scale + this.container.y;
        return { x: screenX, y: screenY };
    }

    getState() {
        return {
            x: this.container.x,
            y: this.container.y,
            zoom: this.scale,
        };
    }

    destroy() {
        // Reset cursor
        this.canvas.style.cursor = "default";

        // Stop animation loop
        if (this.animationFrameId !== null) {
            cancelAnimationFrame(this.animationFrameId);
            this.animationFrameId = null;
        }

        // Remove window listeners if attached
        this.detachWindowListeners();

        // Remove canvas event listeners
        if (this.wheelHandler) {
            this.canvas.removeEventListener("wheel", this.wheelHandler);
        }
        if (this.mouseDownHandler) {
            this.canvas.removeEventListener("mousedown", this.mouseDownHandler);
        }

        // Clear references
        this.wheelHandler = undefined;
        this.mouseDownHandler = undefined;
        this.mouseMoveHandler = undefined;
        this.mouseUpHandler = undefined;
    }
}
