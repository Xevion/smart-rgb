// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;
use tauri::{Manager, RunEvent};

use crate::plugin::{create_game, generate_tauri_context, setup_tauri_resources};
use borders_core::time::Time;

mod analytics;
mod plugin;
mod transport;

const TARGET_FPS: f64 = 60.0;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _guard = tracing::trace_span!("tauri_build").entered();
    let tauri_app = tauri::Builder::default().plugin(tauri_plugin_opener::init()).plugin(tauri_plugin_process::init()).invoke_handler(tauri::generate_handler![transport::register_binary_channel, transport::send_frontend_message, transport::handle_render_input, transport::get_game_state, analytics::track_analytics_event, analytics::flush_analytics, analytics::request_exit,]).build(generate_tauri_context()).expect("error while building tauri application");

    // Phase 1: Setup Tauri resources WITHOUT creating Game/World
    // This registers the message queue so frontend can send messages
    let tauri_resources = setup_tauri_resources(&tauri_app);
    tracing::debug!("Tauri resources ready, waiting for StartGame...");

    // Game doesn't exist yet - will be created when StartGame arrives
    let mut game: Option<borders_core::game::Game> = None;
    let mut tauri_app = tauri_app;

    // Main loop (60 FPS)
    let target_frame_duration = Duration::from_secs_f64(1.0 / TARGET_FPS);

    loop {
        let _guard = tracing::trace_span!("main_frame").entered();

        // Process Tauri events
        #[allow(deprecated)]
        tauri_app.run_iteration(move |_, event: RunEvent| {
            match event {
                RunEvent::Ready => {
                    // Event acknowledged, actual setup happens before loop
                }
                RunEvent::ExitRequested { .. } => {
                    // Track session end and flush analytics before exit
                    if borders_core::telemetry::client().is_some() {
                        tracing::debug!("ExitRequested: tracking session end and flushing analytics");

                        // Create a minimal runtime for blocking operations
                        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().enable_io().build().expect("Failed to create tokio runtime for flush");

                        runtime.block_on(async {
                            // Track session end event
                            borders_core::telemetry::track_session_end().await;

                            // Flush all pending events
                            if let Some(client) = borders_core::telemetry::client() {
                                let timeout = Duration::from_millis(500);
                                match tokio::time::timeout(timeout, client.flush()).await {
                                    Ok(_) => {
                                        tracing::debug!("Analytics flushed successfully before exit")
                                    }
                                    Err(_) => {
                                        tracing::warn!("Analytics flush timed out after 500ms")
                                    }
                                }
                            }
                        });
                    }
                }
                _ => (),
            }
        });

        // Exit if all windows are closed
        if tauri_app.webview_windows().is_empty() {
            tauri_app.cleanup_before_exit();
            break;
        }

        // Phase 2: Create Game when StartGame arrives
        if game.is_none()
            && let Ok(mut messages) = tauri_resources.transport.inbound_messages().lock()
            && let Some(idx) = messages.iter().position(|msg| matches!(msg, borders_core::ui::protocol::FrontendMessage::StartGame))
        {
            tracing::info!("StartGame received, creating Game/World...");
            // Remove StartGame from queue so it's not processed again
            messages.remove(idx);
            drop(messages); // Release lock before creating Game

            // Create the full Game with all plugins and resources
            game = Some(create_game(&tauri_resources));
            tracing::info!("Game/World created, game is ready");
        }

        // Check for QuitGame and drop the Game
        if game.is_some()
            && let Ok(mut messages) = tauri_resources.transport.inbound_messages().lock()
            && let Some(idx) = messages.iter().position(|msg| matches!(msg, borders_core::ui::protocol::FrontendMessage::QuitGame))
        {
            tracing::info!("QuitGame received, dropping Game/World...");
            messages.remove(idx);
            drop(messages);

            // Drop the entire Game/World (no cleanup needed)
            game = None;
            tracing::info!("Game/World dropped, ready for next game");
        }

        // Run game systems only if Game exists
        if let Some(ref mut game_instance) = game {
            // Tick time to measure frame delta
            let frame_start = {
                if let Some(mut time) = game_instance.world_mut().get_resource_mut::<Time>() {
                    time.tick();
                }
                std::time::Instant::now() // For frame rate limiting
            };

            // Run game systems
            game_instance.update();

            // Frame rate limiting
            let frame_duration = frame_start.elapsed();
            if frame_duration < target_frame_duration {
                std::thread::sleep(target_frame_duration - frame_duration);
            }
        } else {
            // No game yet - just sleep to avoid busy waiting
            std::thread::sleep(target_frame_duration);
        }
    }

    std::process::exit(0)
}

fn main() {
    // Initialize tracing before Bevy
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::fmt::format::DefaultFields;
        // Initialize Tracy profiler client
        let _ = tracy_client::Client::start();

        use tracing_subscriber::layer::SubscriberExt;

        struct BareTracyConfig {
            fmt: DefaultFields,
        }

        impl tracing_tracy::Config for BareTracyConfig {
            type Formatter = DefaultFields;

            fn formatter(&self) -> &Self::Formatter {
                &self.fmt
            }

            fn format_fields_in_zone_name(&self) -> bool {
                false
            }
        }

        let tracy_layer = tracing_tracy::TracyLayer::new(BareTracyConfig { fmt: DefaultFields::default() });

        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(tracy_layer)).expect("setup tracy layer");
    }

    #[cfg(not(feature = "tracy"))]
    {
        use tracing_subscriber::fmt::time::FormatTime;
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        #[cfg(debug_assertions)]
        let log_filter = "borders_core=debug,borders_protocol=debug,borders_desktop=debug,iron_borders=debug,info";

        #[cfg(not(debug_assertions))]
        let log_filter = "borders_core=warn,borders_protocol=warn,iron_borders=warn,error";

        struct CustomTimeFormat;

        impl FormatTime for CustomTimeFormat {
            fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
                write!(w, "{}", chrono::Local::now().format("%H:%M:%S%.6f"))
            }
        }

        tracing_subscriber::registry().with(tracing_subscriber::EnvFilter::new(log_filter)).with(tracing_subscriber::fmt::layer().with_timer(CustomTimeFormat)).init();
    }

    // Log build information
    tracing::info!(git_commit = borders_core::build_info::git_commit_short(), build_time = borders_core::build_info::BUILD_TIME, "Iron Borders v{} © 2025 Ryan Walters. All Rights Reserved.", borders_core::build_info::VERSION);

    // Initialize telemetry in background (non-blocking)
    std::thread::spawn(|| {
        let _guard = tracing::trace_span!("telemetry_init").entered();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            borders_core::telemetry::init(borders_core::telemetry::TelemetryConfig::default()).await;
            borders_core::telemetry::track_session_start().await;
            tracing::info!("Observability ready");
        });
    });

    run();
}
