import { Download } from "lucide-react";
import { FeatureCard } from "@/shared/components/menu/views/FeatureCard";

export function DownloadsView() {
    return (
        <div className="col-span-full flex items-center justify-center">
            <div className="w-full max-w-4xl bg-parchment/95 backdrop-blur-xl rounded-xl shadow-2xl p-10 box-border overflow-y-auto">
                <h1 className="font-oswald text-5xl font-bold uppercase m-0 mb-6 text-[#424c4a] tracking-wide text-center">
                    Downloads
                </h1>
                <div className="flex flex-col gap-6">
                    <p className="font-sans text-2xl font-normal opacity-80 m-0 text-center">Standalone applications coming soon...</p>
                    <div className="flex flex-col gap-6 mt-8">
                        <FeatureCard
                            title="Windows"
                            description="Native desktop application for Windows 10+"
                            icon={<Download size={24} />}
                        />
                        <FeatureCard
                            title="macOS"
                            description="Universal binary for Apple Silicon and Intel Macs"
                            icon={<Download size={24} />}
                        />
                        <FeatureCard
                            title="Linux"
                            description="AppImage and Debian packages available"
                            icon={<Download size={24} />}
                        />
                    </div>
                </div>
            </div>
        </div>
    );
}
