use std::ffi::OsString;

use log::{debug, info};
use log4rs::Handle;
use openrgb::OpenRGB;
use windows_service::{
    service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
    service_control_handler::{self, ServiceControlHandlerResult},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

use std::time::Duration;
use tokio::{net::TcpStream, runtime::Runtime, sync::mpsc};

use tokio::sync::mpsc::UnboundedReceiver;
use windows_service::{define_windows_service, service_dispatcher};

const SERVICE_NAME: &str = "RGBXevion";
const SERVICE_DESCRIPTION: &str = "Custom service to toggle RGB lights based on lock/sleep events";

const PROFILE_ENABLE_NAME: &str = "On";
const PROFILE_DISABLE_NAME: &str = "Off";

define_windows_service!(ffi_service_main, service_main);

pub async fn try_load_profile(
    client: &OpenRGB<TcpStream>,
    profile_name: &str,
) -> anyhow::Result<()> {
    let profiles = client.get_profiles().await?;

    let profile_available: bool = profiles.iter().any(|profile| profile == profile_name);
    if !profile_available {
        info!("Profile not found: {}", profile_name);
        return Ok(());
    }

    client.load_profile(profile_name).await?;
    info!("Profile set to: {}", profile_name);

    Ok(())
}

pub(crate) async fn profile_applier(
    profile_recv: &mut UnboundedReceiver<bool>,
    shutdown_recv: &mut UnboundedReceiver<()>,
) -> anyhow::Result<()> {
    let client = OpenRGB::connect().await?;
    client
        .set_name(format!("{} v{}", SERVICE_NAME, env!("CARGO_PKG_VERSION")))
        .await?;

    loop {
        tokio::select! {
            enable = profile_recv.recv() => {
                debug!("Received profile command: {:?}", enable);
                if enable.is_none() {
                    continue;
                }

                try_load_profile(&client, if enable.unwrap() { PROFILE_ENABLE_NAME } else { PROFILE_DISABLE_NAME }).await?;
            }
            _ = shutdown_recv.recv() => {
                info!("Service shutting down");
                return Ok(())
            }
        }
    }
}

#[cfg(windows)]
fn service_main(_: Vec<OsString>) {
    use windows_service::service::{
        PowerEventParam, ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState,
        ServiceStatus, SessionChangeReason,
    };

    let rt = Runtime::new().unwrap();

    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();
    let (profile_send, mut profile_recv) = mpsc::unbounded_channel::<bool>();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::PowerEvent(event) => {
                debug!("Power event: {:?}", event);
                match event {
                    PowerEventParam::QuerySuspend => {
                        // Send false to disable RGB
                        profile_send.send(false).unwrap();
                    }
                    PowerEventParam::ResumeSuspend | PowerEventParam::QuerySuspendFailed => {
                        // Send true to enable RGB
                        profile_send.send(true).unwrap();
                    }
                    _ => {}
                }

                ServiceControlHandlerResult::NoError
            }
            ServiceControl::SessionChange(change) => {
                debug!("Session change: {:?}", change);

                match change.reason {
                    SessionChangeReason::SessionLock => {
                        // Send false to disable RGB
                        profile_send.send(false).unwrap();
                    }
                    SessionChangeReason::SessionUnlock => {
                        // Send true to enable RGB
                        profile_send.send(true).unwrap();
                    }
                    _ => {}
                }

                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_send.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler).unwrap();
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP
                | ServiceControlAccept::SESSION_CHANGE
                | ServiceControlAccept::POWER_EVENT,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .unwrap();

    let error_code = if rt
        .block_on(profile_applier(&mut profile_recv, &mut shutdown_recv))
        .is_err()
    {
        1
    } else {
        0
    };

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(error_code),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .unwrap();
}

fn init_logger() -> Handle {
    use log::LevelFilter;
    use log4rs::{
        append::{console::ConsoleAppender, file::FileAppender},
        config::{Appender, Root},
        encode::pattern::PatternEncoder,
        Config,
    };

    let stdout_appender = ConsoleAppender::builder().build();

    let log_file_path = std::env::current_exe()
        .unwrap()
        .with_file_name("service.log");

    let log_file_appender = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{date} {level} {target} - {message}{n}",
        )))
        .build(log_file_path)
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout_appender)))
        .appender(Appender::builder().build("logfile", Box::new(log_file_appender)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    log4rs::init_config(config).unwrap()
}

#[cfg(windows)]
fn main() -> anyhow::Result<(), windows_service::Error> {
    let _ = init_logger();

    let args = std::env::args().collect::<Vec<_>>();
    let command = args.get(1);

    debug!("Service control executed with args: {:?}", args);

    if let Some(command) = command {
        match command.as_str() {
            "install" => {
                install_service()?;
                info!("Service installed");
                return Ok(());
            }
            "uninstall" => {
                uninstall_service()?;
                info!("Service uninstalled");
                return Ok(());
            }
            "run" => {
                info!("Running service (nil)");
            }
            _ => {
                info!("Unknown command");
                return Ok(());
            }
        }
    }

    info!("Starting service");
    match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        Ok(_) => {}
        Err(e) => {
            info!("Error starting service: {:?}", e);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn install_service() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
    let service_binary_path = ::std::env::current_exe().unwrap();

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec!["run".into()],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };

    let service = service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description(SERVICE_DESCRIPTION)?;

    Ok(())
}

#[cfg(windows)]
fn uninstall_service() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
    let service = service_manager.open_service(SERVICE_NAME, ServiceAccess::DELETE)?;
    service.delete()?;
    Ok(())
}
