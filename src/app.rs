use std::sync::mpsc;

use tracing::{error, info};
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

use crate::config::settings::Settings;
use crate::models::battery_status::BatteryStatus;
use crate::services::battery_service::{spawn_battery_service, ServiceCommand};
use crate::tray::tray_manager::TrayManager;

#[derive(Debug, Clone)]
pub enum AppEvent {
    BatteryUpdated(BatteryStatus),
    MenuEvent(MenuEvent),
    TrayEvent(TrayIconEvent),
}

pub struct InzoneBatteryApp {
    settings: Settings,
    proxy: EventLoopProxy<AppEvent>,
    tray: Option<TrayManager>,
    service_command_tx: Option<mpsc::Sender<ServiceCommand>>,
    service_started: bool,
}

impl InzoneBatteryApp {
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> anyhow::Result<Self> {
        let settings = Settings::load()?;
        Ok(Self {
            settings,
            proxy,
            tray: None,
            service_command_tx: None,
            service_started: false,
        })
    }

    fn ensure_service(&mut self) {
        if self.service_started {
            return;
        }

        TrayIconEvent::set_event_handler({
            let proxy = self.proxy.clone();
            Some(move |event| {
                let _ = proxy.send_event(AppEvent::TrayEvent(event));
            })
        });

        MenuEvent::set_event_handler({
            let proxy = self.proxy.clone();
            Some(move |event| {
                let _ = proxy.send_event(AppEvent::MenuEvent(event));
            })
        });

        let command_tx = spawn_battery_service(self.settings.clone(), self.proxy.clone());
        self.service_command_tx = Some(command_tx);
        self.service_started = true;
    }

    fn handle_battery_update(&mut self, status: BatteryStatus) {
        if let Some(tray) = &mut self.tray {
            if let Err(error) = tray.update_status(&status) {
                error!("failed to update tray state: {error}");
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for InzoneBatteryApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray.is_none() {
            match TrayManager::new() {
                Ok(mut tray) => {
                    let initial = BatteryStatus::disconnected();
                    if let Err(error) = tray.update_status(&initial) {
                        error!("failed to prime tray state: {error}");
                    }
                    self.tray = Some(tray);
                    self.ensure_service();
                    info!("tray initialized");
                }
                Err(error) => {
                    error!("failed to create tray: {error}");
                }
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::BatteryUpdated(status) => self.handle_battery_update(status),
            AppEvent::MenuEvent(event) => {
                if let Some(tray) = &self.tray {
                    if event.id == tray.exit_item_id() {
                        info!("exit requested from tray menu");
                        if let Some(command_tx) = &self.service_command_tx {
                            let _ = command_tx.send(ServiceCommand::Shutdown);
                        }
                        event_loop.exit();
                    }
                }
            }
            AppEvent::TrayEvent(_event) => {}
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }
}
