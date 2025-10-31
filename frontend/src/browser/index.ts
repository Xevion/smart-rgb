/**
 * Browser platform exports.
 * Provides the game bridge instance.
 */

import { GameBridge } from "@/shared/api/GameBridge";
import { WasmTransport } from "@/browser/transport";

const transport = new WasmTransport();

export const bridge = new GameBridge(transport);
