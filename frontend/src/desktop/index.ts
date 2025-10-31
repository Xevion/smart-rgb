/**
 * Desktop platform exports.
 * Provides the game bridge instance.
 */

import { GameBridge } from "@/shared/api/GameBridge";
import { TauriTransport } from "@/desktop/transport";

const transport = new TauriTransport();

export const bridge = new GameBridge(transport);
