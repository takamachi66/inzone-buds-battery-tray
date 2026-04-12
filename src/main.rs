#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod config;
mod hid;
mod models;
mod services;
mod tray;
mod utils;

use app::InzoneBatteryApp;
use cli::Command;
use tracing::{error, info};
use utils::logger::init_logger;
use utils::single_instance::try_acquire;
use winit::event_loop::EventLoop;

fn main() {
    if let Err(error) = run() {
        eprintln!("failed to start application: {error}");
        error!("application startup failed: {error}");
    }
}

fn run() -> anyhow::Result<()> {
    init_logger()?;

    match Command::parse_from_env()? {
        Command::Tray => run_tray(),
        Command::ListHid(args) => cli::run_list_hid(args),
        Command::DumpHid(args) => cli::run_dump_hid(args),
        Command::CompareDumps(args) => cli::run_compare_dumps(args),
        Command::CaptureState(args) => cli::run_capture_state(args),
        Command::CompareStateDirs(args) => cli::run_compare_state_dirs(args),
        Command::CaptureFeatureSeries(args) => cli::run_capture_feature_series(args),
        Command::AnalyzeFeatureSeries(args) => cli::run_analyze_feature_series(args),
        Command::QueryHubBattery(args) => cli::run_query_hub_battery(args),
    }
}

fn run_tray() -> anyhow::Result<()> {
    let Some(_instance_guard) = try_acquire("inzone-buds-battery-tray.instance")? else {
        info!("another instance is already running; skipping startup");
        return Ok(());
    };

    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = InzoneBatteryApp::new(proxy)?;

    event_loop.run_app(&mut app)?;
    Ok(())
}
