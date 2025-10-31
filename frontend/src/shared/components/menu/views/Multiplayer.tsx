import { ArrowLeft } from "lucide-react";
import { FeatureCard } from "@/shared/components/menu/views/FeatureCard";

interface MultiplayerViewProps {
    onBack: () => void;
}

export function MultiplayerView({ onBack }: MultiplayerViewProps) {
    return (
        <div className="w-full min-h-full flex flex-col p-12 box-border bg-gradient-to-br from-[#4591c0] to-[#3c7397] text-white">
            <button
                className="fixed top-8 left-8 flex items-center gap-2 px-5 py-3 bg-white/15 backdrop-blur-lg border-none rounded-lg text-white font-sans text-base font-semibold cursor-pointer transition-all duration-200 z-50 shadow-md hover:bg-white/25 hover:-translate-x-1 hover:shadow-xl active:-translate-x-0.5"
                onClick={onBack}
                aria-label="Back to menu"
            >
                <ArrowLeft size={32} strokeWidth={2.5} />
                <span>Back</span>
            </button>

            <div className="max-w-4xl w-full mx-auto pt-16">
                <h1 className="font-oswald text-7xl font-bold uppercase mb-8 drop-shadow-lg tracking-wide">Multiplayer</h1>
                <div className="flex flex-col gap-8">
                    <p className="font-sans text-2xl font-normal opacity-80 m-0 text-center">Multiplayer lobby coming soon...</p>
                    <div className="flex flex-col gap-6 mt-8">
                        <FeatureCard
                            title="Real-time Matches"
                            description="Challenge players from around the world"
                            variant="multiplayer"
                        />
                        <FeatureCard
                            title="Ranked Play"
                            description="Climb the leaderboard and prove your strategic prowess"
                            variant="multiplayer"
                        />
                        <FeatureCard title="Custom Lobbies" description="Create private matches with friends" variant="multiplayer" />
                    </div>
                </div>
            </div>
        </div>
    );
}
