//! Composes the Phase 1 cloud core into `AccountService` at startup.
//!
//! Cloud composition failures are non-fatal: the tray keeps running and account
//! APIs report signed-out / login unavailable.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use crate::application::account::{AccountService, DesktopLoginConfig};
use crate::application::cloud_session::CloudSession;
use crate::application::ports::cloud_token_store::CloudTokenStore;
use crate::application::ports::desktop_token_exchanger::DesktopTokenExchanger;
use crate::infrastructure::cloud::client::{CloudClient, ReqwestTransport};
use crate::infrastructure::cloud::config::CloudConfig;
use crate::infrastructure::cloud::desktop_token::HttpDesktopTokenExchanger;
use crate::infrastructure::cloud::device_id::DeviceIdentity;
use crate::infrastructure::cloud::logout::HttpCloudRemoteLogout;
use crate::infrastructure::cloud::refresh::HttpCloudTokenRefresher;
use crate::infrastructure::cloud::token_store::KeyringCloudTokenStore;
use crate::platform::system_clock::SystemClock;

pub(crate) struct InstalledAccountRuntime {
    pub service: AccountService,
    pub session: Option<Arc<CloudSession>>,
    pub authenticated_client: Option<Arc<CloudClient>>,
    pub device_id: Option<String>,
    pub device_name: String,
}

pub(crate) fn install_account_service<R: Runtime>(
    app: &AppHandle<R>,
    app_version: &str,
) -> InstalledAccountRuntime {
    let device_name = resolve_device_name();
    let (device_id, device_name) = match app.path().app_data_dir() {
        Ok(dir) => {
            let identity = DeviceIdentity::new(dir, device_name.clone());
            match identity.get_or_create_device_id() {
                Ok(id) => (Some(id), identity.device_name().to_owned()),
                Err(error) => {
                    eprintln!("Burnly cloud device id unavailable: {error}");
                    (None, device_name)
                }
            }
        }
        Err(error) => {
            eprintln!("Burnly cloud device id path unavailable: {error}");
            (None, device_name)
        }
    };

    let login_config = match CloudConfig::from_env(app_version) {
        Ok(config) => Some(DesktopLoginConfig {
            web_origin: config.web_origin().to_owned(),
            redirect_uri: config.redirect_uri().to_owned(),
        }),
        Err(error) => {
            eprintln!("Burnly cloud config unavailable: {error}");
            None
        }
    };

    match compose_cloud_stack(app_version) {
        Ok(stack) => {
            if let Err(error) = stack.session.restore() {
                eprintln!("Burnly cloud session restore failed: {error}");
            }
            match login_config {
                Some(config) => {
                    let exchanger: Arc<dyn DesktopTokenExchanger> =
                        Arc::new(HttpDesktopTokenExchanger::new(stack.public_client));
                    InstalledAccountRuntime {
                        service: AccountService::from_session(
                            stack.session.clone(),
                            device_id.clone(),
                            device_name.clone(),
                            config,
                            exchanger,
                        ),
                        session: Some(stack.session),
                        authenticated_client: Some(stack.authenticated_client),
                        device_id,
                        device_name,
                    }
                }
                None => InstalledAccountRuntime {
                    service: AccountService::unavailable(
                        device_id.clone(),
                        device_name.clone(),
                        None,
                    ),
                    session: Some(stack.session),
                    authenticated_client: Some(stack.authenticated_client),
                    device_id,
                    device_name,
                },
            }
        }
        Err(error) => {
            eprintln!("Burnly cloud account runtime unavailable: {error}");
            InstalledAccountRuntime {
                service: AccountService::unavailable(
                    device_id.clone(),
                    device_name.clone(),
                    login_config,
                ),
                session: None,
                authenticated_client: None,
                device_id,
                device_name,
            }
        }
    }
}

struct CloudStack {
    session: Arc<CloudSession>,
    public_client: Arc<CloudClient>,
    authenticated_client: Arc<CloudClient>,
}

fn compose_cloud_stack(app_version: &str) -> Result<CloudStack, String> {
    let config = CloudConfig::from_env(app_version).map_err(|error| error.to_string())?;
    let transport = Arc::new(ReqwestTransport::new().map_err(|error| error.message.clone())?);
    let clock = Arc::new(SystemClock);
    let public_client = Arc::new(CloudClient::new(
        config.clone(),
        transport.clone(),
        None,
        clock.clone(),
    ));
    let refresher = Arc::new(HttpCloudTokenRefresher::new(public_client.clone()));
    let remote_logout = Arc::new(HttpCloudRemoteLogout::new(public_client.clone()));
    let store: Arc<dyn CloudTokenStore> =
        Arc::new(KeyringCloudTokenStore::new().map_err(|_| "keyring unavailable".to_owned())?);

    let session = Arc::new(CloudSession::new(
        store,
        refresher,
        remote_logout,
        clock.clone(),
    ));
    let authenticated_client = Arc::new(CloudClient::new(
        config,
        transport,
        Some(session.clone()),
        clock,
    ));
    Ok(CloudStack {
        session,
        public_client,
        authenticated_client,
    })
}

fn resolve_device_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Burnly Desktop".to_owned())
}
