use std::ffi::OsString;

use windows_service::{
    service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
    service_manager::{ServiceManager, ServiceManagerAccess},
    service_control_handler::{self, ServiceControlHandlerResult},
};

use std::time::Duration;
use tokio::{runtime::Runtime, sync::mpsc};

use tokio::sync::mpsc::UnboundedReceiver;
use windows_service::{define_windows_service, service_dispatcher};

const SERVICE_NAME: &str = "Easy RGB - Background Scheduler";
const SERVICE_DESCRIPTION: &str = "Service to apply rules to background processes";

define_windows_service!(ffi_service_main, service_main);

pub(crate) async fn rule_applier(
    rule_file_path: &str,
    shutdown_recv: &mut UnboundedReceiver<()>,
) -> anyhow::Result<()> {
    // let wmi_con = WMIConnection::new(COMLibrary::new()?)?;

    // Apply rules to all running processes
    // let running_process: Vec<WinProcess> = wmi_con.async_query().await?;
    // running_process.into_iter().for_each(|process| {
    //     let process_info: ProcessInfo = process.into();
    //     rule_set.apply(&process_info)
    // });

    tokio::select! {
        // Apply rules to new processes
        // output = monitor_new_processes(&rule_set, &wmi_con) => output,
        // Or wait for shutdown signal
        _ = shutdown_recv.recv() => {
            println!("Shutting down process monitor");
            Ok(())
        }
    }
}

#[cfg(windows)]
fn service_main(_: Vec<OsString>) {
    use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus};



    let rt = Runtime::new().unwrap();
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
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
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .unwrap();

    let args = std::env::args().collect::<Vec<_>>();
    let rules_path = args.get(2).map(|s| s.as_str()).unwrap_or("rules.json");

    let error_code = if rt
        .block_on(rule_applier(rules_path, &mut shutdown_recv))
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

#[cfg(windows)]
fn main() -> anyhow::Result<(), windows_service::Error> {
    use log::{debug, info, LevelFilter};
    use log4rs::{append::{console::ConsoleAppender, file::FileAppender}, config::{Appender, Logger, Root}, encode::pattern::PatternEncoder, Config};

    let stdout = ConsoleAppender::builder().build();

    let log_file_path = std::env::current_exe().unwrap().with_file_name("service.log");

    let requests = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::default()))
        .build(log_file_path)
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("requests", Box::new(requests)))
        .logger(Logger::builder().build("app::backend::db", LevelFilter::Debug))
        .logger(Logger::builder()
            .appender("requests")
            .additive(false)
            .build("app::requests", LevelFilter::Info))
        .build(Root::builder().appender("stdout").appender("requests").build(LevelFilter::Trace))
        .unwrap();

    let handle = log4rs::init_config(config).unwrap();

    let args = std::env::args().collect::<Vec<_>>();
    let command = args.get(1);

    debug!("Service control executed with args: {:?}", args);

    if let Some(command) = command {
        match command.as_str() {
            "install" => {
                install_service(args.get(2).map(|s| s.as_str()))?;
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

    println!("Starting service");
    match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        Ok(_) => {}
        Err(e) => {
            println!("Error starting service: {:?}", e);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn install_service(rules_path: Option<&str>) -> windows_service::Result<()> {
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
