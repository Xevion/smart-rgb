import * as Dialog from "@radix-ui/react-dialog";
import { BlurOverlay } from "@/shared/components/overlays/BlurOverlay";

interface AlphaWarningModalProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

export function AlphaWarningModal({ open, onOpenChange }: AlphaWarningModalProps) {
    return (
        <BlurOverlay open={open} onOpenChange={onOpenChange}>
            <div className="flex flex-col items-center justify-center max-w-lg text-center gap-6 select-none mx-6">
                <Dialog.Title className="font-oswald text-6xl font-extrabold uppercase tracking-wide text-white text-shadow-md">
                    Alpha Software
                </Dialog.Title>
                <Dialog.Description className="text-2xl leading-7 text-slate-100 text-shadow-sm">
                    This application is in early development. Expect bugs, missing features, and breaking changes. Your feedback is
                    appreciated as we continue building.
                </Dialog.Description>
            </div>
        </BlurOverlay>
    );
}

export default AlphaWarningModal;
