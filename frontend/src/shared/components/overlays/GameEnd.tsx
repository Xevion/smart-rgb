import type { GameOutcome } from "@/shared/api/types";

interface GameEndOverlayProps {
    outcome: GameOutcome;
    onSpectate: () => void;
    onExit: () => void;
}

export function GameEndOverlay({ outcome, onSpectate, onExit }: GameEndOverlayProps) {
    return (
        <div className="fixed top-0 left-0 right-0 bottom-0 flex items-center justify-center z-1000 pointer-events-none">
            <div className="pointer-events-auto user-select-none w-80 text-sm">
                {/* Victory/Defeat text */}
                <div className="bg-slate-900/75 p-6 text-center rounded-t-md">
                    <div className="font-oswald text-2xl font-medium mb-2">{outcome === "Victory" ? "Victory" : "Defeat"}</div>
                    {/* TODO: Make subtitle dynamic based on win/loss condition:
              - Victory by elimination: "You destroyed all other Nations."
              - Victory by occupation: "You reached 80% occupation of the map."
              - Defeat by elimination: "Your nation was eradicated."
              - Defeat by enemy occupation: "{NationName} reached 80% occupation of the map."
          */}
                    <div className="font-inter text-lg font-medium opacity-85">
                        {outcome === "Victory" ? "You conquered the map." : "Your nation fell."}
                    </div>
                </div>

                {/* Button row */}
                <div className="flex bg-slate-900/60 rounded-b-md">
                    <button
                        className="bg-transparent flex-1 p-3 border-none text-white font-inter text-lg font-medium transition-colors duration-150 hover:bg-white/6 cursor-pointer"
                        onClick={onSpectate}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.background = "rgba(255, 255, 255, 0.06)";
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.background = "transparent";
                        }}
                    >
                        Spectate
                    </button>
                    <button
                        className="bg-transparent flex-1 p-3 border-none text-white font-inter text-lg font-medium transition-colors duration-150 hover:bg-white/6 cursor-pointer"
                        onClick={onExit}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.background = "rgba(255, 255, 255, 0.06)";
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.background = "transparent";
                        }}
                    >
                        Exit
                    </button>
                </div>
            </div>
        </div>
    );
}

export default GameEndOverlay;
