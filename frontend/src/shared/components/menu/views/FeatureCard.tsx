import { cn } from "@/lib/utils";

export interface FeatureCardProps {
    title: string;
    description: string;
    icon?: React.ReactNode;
    variant?: "default" | "multiplayer";
    className?: string;
}

export function FeatureCard({ title, description, icon, variant = "default", className }: FeatureCardProps) {
    return (
        <div
            className={cn(
                "p-8 rounded-xl shadow-lg transition-all duration-200",
                "hover:-translate-y-0.5 hover:shadow-xl",
                variant === "multiplayer" && "bg-white/10 backdrop-blur-sm",
                className,
            )}
        >
            {icon ? (
                <div className="flex items-center gap-2 mb-2">
                    {icon}
                    <h3 className="font-oswald text-3xl md:text-2xl font-semibold uppercase tracking-tight m-0">{title}</h3>
                </div>
            ) : (
                <h3 className="font-oswald text-3xl md:text-2xl font-semibold uppercase tracking-tight m-0 mb-2">{title}</h3>
            )}
            <p className="font-sans text-base md:text-sm m-0 opacity-90 leading-normal">{description}</p>
        </div>
    );
}
