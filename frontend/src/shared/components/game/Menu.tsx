import { useState } from "react";
import { Menu, X } from "lucide-react";
import { cn } from "@/lib/utils";

interface GameMenuProps {
    onExit: () => void;
    onSettings?: () => void;
}

export function GameMenu({ onExit, onSettings }: GameMenuProps) {
    const [isOpen, setIsOpen] = useState(false);
    const [showConfirmation, setShowConfirmation] = useState(false);
    const [isClosing, setIsClosing] = useState(false);

    const closeConfirmation = () => {
        setIsClosing(true);
        setTimeout(() => {
            setShowConfirmation(false);
            setIsClosing(false);
        }, 200);
    };

    return (
        <div className="absolute top-4 right-4">
            {/* pointer-events-auto: Required if this component is rendered inside a pointer-events-none container */}
            <button
                className="bg-slate-900/75 border-none rounded-md text-white p-2 cursor-pointer pointer-events-auto flex items-center justify-center transition-colors duration-150 hover:bg-slate-900/90"
                onClick={() => setIsOpen((prev) => !prev)}
            >
                {isOpen ? <X size={20} /> : <Menu size={20} />}
            </button>
            {isOpen && (
                <div className="absolute top-full mt-2 right-0 bg-slate-900/75 rounded-md min-w-36 overflow-hidden">
                    <button
                        className={cn(
                            "w-full bg-transparent border-none text-white py-1.5 px-4 text-right cursor-pointer font-inter transition-colors duration-150 hover:bg-white/6",
                            { "border-t border-white/10": onSettings },
                        )}
                        onClick={() => {
                            setShowConfirmation(true);
                            setIsClosing(false);
                            setIsOpen(false);
                        }}
                    >
                        Exit
                    </button>
                </div>
            )}

            {showConfirmation && (
                <div
                    className={cn(
                        "fixed inset-0 bg-black/50 flex items-center justify-center",
                        isClosing ? "animate-[fadeOut_forwards] duration-200 ease-out" : "animate-[fadeIn] duration-200 ease-out",
                    )}
                    onClick={closeConfirmation}
                >
                    <div
                        className={cn(
                            "bg-zinc-900 rounded-lg p-8 min-w-80 max-w-96 shadow-lg shadow-zinc-950/50",
                            isClosing
                                ? "animate-[slideDown_forwards] duration-200 ease-out"
                                : "animate-[slideUp] duration-200 ease-out",
                        )}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <h3 className="mb-4 text-white text-xl font-inter font-semibold">Are you sure?</h3>
                        <p className="mb-6 text-white/80  font-inter leading-normal text-pretty">
                            You will not be able to return to this game after exiting.
                        </p>
                        <div className="flex gap-3 justify-end">
                            <button
                                className="border-none rounded-md text-white px-5 cursor-pointer font-inter transition-colors duration-150 ease-in-out bg-white/10 hover:bg-white/15"
                                onClick={closeConfirmation}
                            >
                                Nevermind
                            </button>
                            <button
                                className="border-none rounded-md text-white px-5 cursor-pointer font-inter transition-colors duration-150 bg-red-500 font-medium hover:bg-red-400"
                                onClick={() => {
                                    setIsClosing(true);
                                    setTimeout(() => {
                                        setShowConfirmation(false);
                                        setIsClosing(false);
                                        onExit();
                                    }, 200);
                                }}
                            >
                                I'm sure
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
