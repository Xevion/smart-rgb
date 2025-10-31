import { useState, useEffect, useRef, lazy, Suspense } from "react";
import { motion, AnimatePresence, Variants } from "motion/react";
import { Power } from "lucide-react";
import { OverlayScrollbarsComponent } from "overlayscrollbars-react";
import { MultiplayerView } from "@/shared/components/menu/views/Multiplayer";
import { SettingsView } from "@/shared/components/menu/views/Settings";
import { DownloadsView } from "@/shared/components/menu/views/Downloads";
import inlineScreenshot from "@/assets/images/screenshot.png?w=100&format=webp&inline&imagetools";
import singleplayerImage from "@/assets/images/menu/singleplayer.png";
import multiplayerImage from "@/assets/images/menu/multiplayer.png";
import settingsImage from "@/assets/images/menu/settings.png";
import "overlayscrollbars/overlayscrollbars.css";
import "@/shared/styles/animations.css";
import { cn } from "@/lib/utils";

const AlphaWarningModal = lazy(() => import("@/shared/components/overlays/AlphaWarning"));

type MenuView = "home" | "multiplayer" | "settings" | "downloads";

// Animation constants
const TRANSITION_CONFIG = {
    duration: 0.25,
    ease: [0.4, 0, 0.2, 1] as [number, number, number, number],
};

const slideVariants: Variants = {
    homeEnter: { x: "-200%" },
    contentEnter: { x: "200%" },
    homeCenter: { x: 0 },
    contentCenter: { x: 0 },
    homeExit: { x: "-200%" },
    contentExit: { x: "200%" },
};

const cardVariants: Variants = {
    cardEnter: { y: "100%", opacity: 0 },
    cardCenter: { y: 0, opacity: 1 },
    cardExit: { y: "100%", opacity: 0 },
};

const footerVariants: Variants = {
    visible: { y: 0, opacity: 1 },
    hidden: { y: 100, opacity: 0 },
};

const titleVariants: Variants = {
    visible: { y: 0, opacity: 1 },
    hidden: { y: -100, opacity: 0 },
};

const buttonExitVariants: Variants = {
    visible: { scale: 1, opacity: 1 },
    hidden: { scale: 0.8, opacity: 0 },
};

const backgroundVariants: Variants = {
    visible: { opacity: 1 },
    loading: { opacity: 1 },
    exit: { opacity: 0 },
};

// Loading percentage constants
const PROGRESS_RATE = 3; // Controls curve speed - higher = faster approach
const PROGRESS_CAP = 95; // Max % before game ready (never naturally reaches 100%)
const FINAL_JUMP_DURATION = 0.2; // Time to jump to 100% when game ready (seconds)

// Shared button styles
const BASE_BUTTON_CLASSES =
    "border-none rounded-md cursor-pointer transition-all duration-300 relative overflow-hidden select-none px-4 py-2 text-left shadow-xl flex flex-col justify-start items-start";
const BUTTON_HOVER_CLASSES =
    "hover:enabled:-translate-y-1 hover:enabled:shadow-2xl active:enabled:-translate-y-0.5 disabled:cursor-default";

// Internal Components
interface MenuBackgroundProps {
    state: "visible" | "loading" | "exit";
    duration: number;
    onExitComplete?: () => void;
}

function MenuBackground({ state, duration, onExitComplete }: MenuBackgroundProps) {
    return (
        <motion.div
            className="absolute inset-0 w-full h-full bg-gradient-to-br from-slate-50 via-slate-200 to-slate-300 overflow-hidden"
            initial="visible"
            animate={state}
            variants={backgroundVariants}
            transition={{
                duration: state === "exit" ? duration : 0,
                ease: [0.4, 0, 0.2, 1],
            }}
            onAnimationComplete={(variant) => {
                if (variant === "exit") {
                    onExitComplete?.();
                }
            }}
        >
            <img
                src={inlineScreenshot}
                alt="Menu background"
                className="absolute w-full h-full object-cover blur-[1px] brightness-65 sepia-30 contrast-150 scale-105"
            />
            <div className="absolute inset-0 bg-white/40 backdrop-blur"></div>
        </motion.div>
    );
}

interface LoadingPercentageProps {
    isLoading: boolean;
    gameReady: boolean;
}

function LoadingPercentage({ isLoading, gameReady }: LoadingPercentageProps) {
    const [progress, setProgress] = useState(0);
    const startTimeRef = useRef<number | null>(null);
    const jumpStartTimeRef = useRef<number | null>(null);
    const jumpStartProgressRef = useRef<number>(0);
    const animationFrameRef = useRef<number | null>(null);
    const gameReadyRef = useRef(false);

    useEffect(() => {
        gameReadyRef.current = gameReady;
    }, [gameReady]);

    useEffect(() => {
        if (!isLoading) {
            setProgress(0);
            startTimeRef.current = null;
            jumpStartTimeRef.current = null;
            jumpStartProgressRef.current = 0;
            gameReadyRef.current = false;
            if (animationFrameRef.current) {
                cancelAnimationFrame(animationFrameRef.current);
                animationFrameRef.current = null;
            }
            return;
        }

        startTimeRef.current = Date.now();

        const animate = () => {
            if (!startTimeRef.current) return;

            if (gameReadyRef.current && !jumpStartTimeRef.current) {
                // Game just became ready, start final jump
                jumpStartTimeRef.current = Date.now();
                jumpStartProgressRef.current =
                    PROGRESS_CAP * (1 - Math.exp(-PROGRESS_RATE * ((Date.now() - startTimeRef.current) / 1000)));
            }

            if (jumpStartTimeRef.current) {
                // Animate final jump to 100%
                const elapsed = (Date.now() - jumpStartTimeRef.current) / 1000;
                const t = Math.min(elapsed / FINAL_JUMP_DURATION, 1);
                const newProgress = jumpStartProgressRef.current + (100 - jumpStartProgressRef.current) * t;

                setProgress(newProgress);

                if (t < 1) {
                    animationFrameRef.current = requestAnimationFrame(animate);
                } else {
                    setProgress(100);
                }
            } else {
                // Exponential curve animation
                const elapsed = (Date.now() - startTimeRef.current) / 1000;
                const newProgress = PROGRESS_CAP * (1 - Math.exp(-PROGRESS_RATE * elapsed));

                setProgress(newProgress);
                animationFrameRef.current = requestAnimationFrame(animate);
            }
        };

        animate();

        return () => {
            if (animationFrameRef.current) {
                cancelAnimationFrame(animationFrameRef.current);
            }
        };
    }, [isLoading]);

    if (!isLoading) return null;

    const value = Math.round(progress);
    const hundreds = value >= 100 ? "1" : "";
    const tens = value >= 10 ? Math.floor((value % 100) / 10).toString() : "";
    const ones = (value % 10).toString();

    const slotStyle = {
        fontSize: "clamp(15rem, 35vw, 40rem)",
        opacity: 0.15,
        textShadow: "2px 2px 4px rgba(0, 0, 0, 0.4), -1px -1px 2px rgba(255, 255, 255, 0.1)",
        width: "1ch",
        textAlign: "center" as const,
    };

    return (
        <motion.div
            className="absolute inset-0 flex items-center justify-center pointer-events-none"
            initial={{ opacity: 1 }}
            animate={{ opacity: gameReadyRef.current ? 0 : 1 }}
            transition={{ duration: gameReadyRef.current ? 0.8 : 0 }}
        >
            <div className="flex font-oswald font-bold text-white select-none leading-none" style={{ marginRight: "clamp(10.5rem, 24.5vw, 28rem)" }}>
                <span style={slotStyle}>{hundreds}</span>
                <span style={slotStyle}>{tens}</span>
                <span style={slotStyle}>{ones}</span>
                <span style={slotStyle}>%</span>
            </div>
        </motion.div>
    );
}

interface MenuHeaderProps {
    shouldHide: boolean;
    duration: number;
}

function MenuHeader({ shouldHide, duration }: MenuHeaderProps) {
    return (
        // Click-through header: pointer-events-none prevents blocking interactions with elements below
        <motion.div
            className="flex flex-col items-center gap-2 w-full py-8 mt-8 bg-zinc-900/45 backdrop-blur-lg backdrop-contrast-150 pointer-events-none"
            initial="visible"
            animate={shouldHide ? "hidden" : "visible"}
            variants={titleVariants}
            transition={{ duration, ease: TRANSITION_CONFIG.ease }}
        >
            <h1 className="font-oswald uppercase text-8xl font-semibold select-none text-white m-0 leading-[0.8] text-shadow-lg animate-[shimmer] duration-3000 ease-in-out">
                Iron Borders
            </h1>
        </motion.div>
    );
}

interface MenuFooterProps {
    shouldHide: boolean;
    onExit: () => void;
    onVersionClick: () => void;
    duration: number;
}

function MenuFooter({ shouldHide, onExit, onVersionClick, duration }: MenuFooterProps) {
    return (
        // Click-through container: pointer-events-none allows clicks through to menu cards.
        // Interactive children (buttons, links) use pointer-events-auto to re-enable interaction.
        <motion.div
            className="fixed bottom-6 inset-x-0 flex justify-between items-end pr-12 pl-8 pointer-events-none z-50"
            initial="visible"
            animate={shouldHide ? "hidden" : "visible"}
            variants={footerVariants}
            transition={{ duration, ease: TRANSITION_CONFIG.ease }}
        >
            <div className="flex items-end">
                {__DESKTOP__ && (
                    <button
                        className="flex items-center justify-center bg-transparent border-none cursor-pointer transition-all duration-200 text-white opacity-85 mix-blend-screen pointer-events-auto drop-shadow-lg hover:opacity-100 hover:scale-110 hover:drop-shadow-md active:scale-90 focus-visible:outline-2 focus-visible:outline-white/50 focus-visible:outline-offset-4"
                        onClick={onExit}
                    >
                        <Power size={40} strokeWidth={2.5} />
                    </button>
                )}
            </div>
            <div className="flex flex-col items-end">
                <button
                    className="font-oswald text-3xl font-bold p-0 bg-transparent text-white border-none cursor-pointer transition-all duration-200 pointer-events-auto select-none uppercase tracking-tight inline-block relative shadow-none drop-shadow-md hover:-translate-y-px active:translate-y-0 active:bg-transparent leading-4"
                    onClick={onVersionClick}
                    title={`Git: ${__GIT_COMMIT__.substring(0, 7)}\nBuild: ${new Date(__BUILD_TIME__).toLocaleString()}`}
                >
                    <span className="drop-shadow-md">
                        {__APP_VERSION__ === "0.0.0" ? "Unknown Version" : `v${__APP_VERSION__.trimEnd()}`}
                    </span>
                    <span className="text-[#ffc56d] absolute -right-3 top-0 drop-shadow-md">*</span>
                </button>
                <div className="pointer-events-auto select-none">
                    <a
                        className="text-shadow-md text-gray-50 hover:text-gray-100 transition-colors duration-200"
                        href="https://github.com/Xevion"
                        target="_blank"
                    >
                        © 2025 Ryan Walters
                    </a>
                </div>
            </div>
        </motion.div>
    );
}

type ButtonColorScheme = "orange" | "blue" | "beige";

interface MenuButtonProps {
    title: string;
    description: string;
    onClick: () => void;
    colorScheme: ButtonColorScheme;
    image?: string;
    imagePosition?: "left" | "right";
    className?: string;
    disabled?: boolean;
}

function MenuButton({
    title,
    description,
    onClick,
    colorScheme,
    image,
    imagePosition = "right",
    className,
    disabled = false,
}: MenuButtonProps) {
    const colorClasses = {
        orange: "bg-[#c77a06] text-white active:enabled:bg-[#a66305]",
        blue: "bg-[#4591c0] text-white active:enabled:bg-[#3c7397]",
        beige: "bg-[#f1ebdb] text-[#424c4a] active:enabled:bg-[#d9d1bb]",
    };

    const hasWhiteText = colorScheme === "orange" || colorScheme === "blue";

    return (
        <button
            className={cn(
                BASE_BUTTON_CLASSES,
                BUTTON_HOVER_CLASSES,
                colorClasses[colorScheme],
                { BUTTON_TEXT_SHADOW: hasWhiteText },
                className,
            )}
            onClick={onClick}
            disabled={disabled}
        >
            {image && (
                <img
                    src={image}
                    alt=""
                    className={cn(
                        "absolute bottom-0 w-full h-auto object-contain pointer-events-none z-0 drop-shadow-lg [image-rendering:pixelated]",
                        imagePosition === "left" ? "left-0" : "right-0",
                    )}
                />
            )}
            <div>
                <h2 className="font-oswald text-[2rem] font-bold mb-1 uppercase tracking-wide">{title}</h2>
                <p className="leading-6 opacity-95">{description}</p>
            </div>
        </button>
    );
}

interface MenuScreenProps {
    onStartSingleplayer: () => void;
    onExit: () => void;
    isExiting?: boolean;
    gameReady?: boolean;
    onExitComplete?: () => void;
}

export function MenuScreen({ onStartSingleplayer, onExit, isExiting = false, gameReady = false, onExitComplete }: MenuScreenProps) {
    const [activeView, setActiveView] = useState<MenuView>("home");
    const [isModalOpen, setIsModalOpen] = useState(false);
    const [transitionStartTime, setTransitionStartTime] = useState<number | null>(null);

    // Handle browser back/forward navigation
    useEffect(() => {
        const handlePopState = (event: PopStateEvent) => {
            const view = (event.state?.view as MenuView) || "home";
            setActiveView(view);
        };

        window.addEventListener("popstate", handlePopState);
        return () => window.removeEventListener("popstate", handlePopState);
    }, []);

    const handleSingleplayerClick = () => {
        setTransitionStartTime(Date.now());
        onStartSingleplayer();
    };

    const handleNavigate = (view: MenuView) => {
        setActiveView(view);
        window.history.pushState({ view }, "");
    };

    const handleBackToHome = () => {
        setActiveView("home");
        window.history.replaceState({ view: "home" }, "");
    };

    // Calculate transition timings based on load speed
    const loadTime = transitionStartTime && gameReady ? Date.now() - transitionStartTime : null;
    const isFastLoad = loadTime !== null && loadTime < 500;

    // Transition durations (in seconds)
    const componentExitDuration = isFastLoad ? 0.15 : 0.25;
    const backgroundFadeDuration = isFastLoad ? 0.5 : 0.8;

    // Determine current state for animations
    const shouldHideChrome = activeView === "multiplayer" || isExiting;
    const backgroundState = !isExiting ? "visible" : gameReady ? "exit" : "loading";

    const handleBackgroundExitComplete = () => {
        if (gameReady && backgroundState === "exit") {
            onExitComplete?.();
        }
    };

    return (
        <div className="fixed inset-0 flex items-center justify-center select-none z-50">
            <MenuBackground state={backgroundState} duration={backgroundFadeDuration} onExitComplete={handleBackgroundExitComplete} />
            <LoadingPercentage isLoading={backgroundState === "loading" || backgroundState === "exit"} gameReady={gameReady} />

            <OverlayScrollbarsComponent
                className="absolute inset-0 w-full h-full z-1 overflow-x-hidden"
                defer
                options={{ scrollbars: { autoHide: "scroll" }, overflow: { x: "hidden" } }}
            >
                <div className="flex flex-col items-center text-center w-full min-h-full">
                    <MenuHeader shouldHide={shouldHideChrome} duration={componentExitDuration} />

                    <AnimatePresence mode="wait" initial={false}>
                        {activeView === "home" ? (
                            <motion.div
                                key="home"
                                className="grid grid-cols-[1fr_2fr] gap-4 w-full max-w-3xl my-12 mx-auto pointer-events-auto"
                                initial="homeEnter"
                                animate={isExiting ? "hidden" : "homeCenter"}
                                exit="homeExit"
                                variants={{ ...slideVariants, ...buttonExitVariants }}
                                transition={{
                                    duration: isExiting ? componentExitDuration : TRANSITION_CONFIG.duration,
                                    ease: TRANSITION_CONFIG.ease,
                                }}
                            >
                                <MenuButton
                                    title="Local"
                                    description="Battle against AI opponents in strategic territorial warfare"
                                    onClick={handleSingleplayerClick}
                                    colorScheme="orange"
                                    image={singleplayerImage}
                                    imagePosition="left"
                                    className="rounded-md shadow-lg aspect-[1/2]"
                                    disabled={isExiting}
                                />

                                <div className="flex flex-col gap-[1.05rem] aspect-square">
                                    <MenuButton
                                        title="Multiplayer"
                                        description="Challenge other players in real-time matches"
                                        onClick={() => handleNavigate("multiplayer")}
                                        colorScheme="blue"
                                        image={multiplayerImage}
                                        className="flex-1"
                                    />

                                    <MenuButton
                                        title="Settings"
                                        description="Customize your experience"
                                        onClick={() => handleNavigate("settings")}
                                        colorScheme="beige"
                                        image={settingsImage}
                                        className="flex-1"
                                    />
                                </div>

                                {!__DESKTOP__ && (
                                    <MenuButton
                                        title="Downloads"
                                        description="Get standalone versions for Windows, Mac, and Linux"
                                        onClick={() => handleNavigate("downloads")}
                                        colorScheme="beige"
                                        className="col-span-full"
                                    />
                                )}
                            </motion.div>
                        ) : activeView === "multiplayer" ? (
                            <motion.div
                                key="multiplayer"
                                className="absolute inset-0 w-full h-full min-h-full"
                                initial="contentEnter"
                                animate="contentCenter"
                                exit="contentExit"
                                variants={slideVariants}
                                transition={TRANSITION_CONFIG}
                            >
                                <MultiplayerView onBack={handleBackToHome} />
                            </motion.div>
                        ) : (
                            <motion.div
                                key={activeView}
                                className="grid grid-cols-[1fr_2fr] gap-4 w-full max-w-3xl my-12 mx-auto pointer-events-auto"
                                initial="cardEnter"
                                animate="cardCenter"
                                exit="cardExit"
                                variants={cardVariants}
                                transition={TRANSITION_CONFIG}
                            >
                                {activeView === "settings" && <SettingsView />}
                                {activeView === "downloads" && <DownloadsView />}
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>
            </OverlayScrollbarsComponent>

            <MenuFooter
                shouldHide={shouldHideChrome}
                onExit={onExit}
                onVersionClick={() => setIsModalOpen(true)}
                duration={componentExitDuration}
            />

            <Suspense fallback={null}>
                <AlphaWarningModal open={isModalOpen} onOpenChange={setIsModalOpen} />
            </Suspense>
        </div>
    );
}
