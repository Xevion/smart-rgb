import { FeatureCard } from "@/shared/components/menu/views/FeatureCard";

export function SettingsView() {
    return (
        <div className="col-span-full flex items-center justify-center">
            <div className="w-full max-w-4xl bg-parchment/95 backdrop-blur-xl rounded-xl shadow-2xl p-10 box-border overflow-y-auto">
                <h1 className="font-oswald text-5xl font-bold uppercase m-0 mb-6 text-[#424c4a] tracking-wide text-center">
                    Settings
                </h1>
                <div className="flex flex-col gap-6">
                    <p className="font-sans text-2xl font-normal opacity-80 m-0 text-center">Settings panel coming soon...</p>
                    <div className="flex flex-col gap-6 mt-8">
                        <FeatureCard title="Graphics" description="Adjust visual quality and performance" />
                        <FeatureCard title="Audio" description="Configure sound effects and music volume" />
                        <FeatureCard title="Controls" description="Customize keybindings and input preferences" />
                        <FeatureCard title="Accessibility" description="Enable colorblind modes and UI scaling" />
                    </div>
                </div>
            </div>
        </div>
    );
}
