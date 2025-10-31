import { X } from "lucide-react";
import * as Dialog from "@radix-ui/react-dialog";
import { MouseEvent, ReactNode, useMemo } from "react";
import "@/shared/styles/animations.css";

interface BlurOverlayProps {
    open: boolean;
    onOpenChange?: (open: boolean) => void;
    children: ReactNode;
    showCloseButton?: boolean;
}

export function BlurOverlay({ open, onOpenChange, children, showCloseButton = true }: BlurOverlayProps) {
    const handleBackdropClick = (e: MouseEvent) => {
        if (e.target === e.currentTarget) {
            onOpenChange?.(false);
        }
    };

    const closeButton = useMemo(() => {
        return (
            <div className="absolute top-12 right-12 max-md:top-6 max-md:right-6">
                <Dialog.Close asChild>
                    <button
                        className="flex items-center justify-center cursor-pointer transition-all duration-200 text-white opacity-60 mix-blend-screen hover:opacity-100 hover:scale-110 active:scale-90 focus:outline-none focus-visible:outline-2 focus-visible:outline-white/50 focus-visible:outline-offset-4"
                        aria-label="Close"
                    >
                        <X size={64} strokeWidth={2.5} />
                    </button>
                </Dialog.Close>
            </div>
        );
    }, []);

    return (
        <Dialog.Root open={open} onOpenChange={onOpenChange ?? undefined}>
            <Dialog.Portal>
                <Dialog.Overlay className="fixed inset-0 bg-black/50 fade-blur-in" />
                <Dialog.Content
                    className="fixed inset-0 flex flex-col items-center justify-center outline-none fade-in"
                    onPointerDown={handleBackdropClick}
                >
                    {showCloseButton && closeButton}
                    {children}
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

export default BlurOverlay;
