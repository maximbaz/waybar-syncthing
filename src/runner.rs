use crate::api_client::ApiClient;
use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, fmt};

#[derive(Debug)]
pub struct Runner {
    client: ApiClient,
    devices: HashMap<DeviceID, DeviceName>,
    folders: HashMap<FolderID, FolderName>,
    pending: HashMap<DeviceID, HashMap<FolderID, (ProgressPct, NeedBytes)>>,
    since: u64,
}

impl Runner {
    pub fn new(client: ApiClient) -> Self {
        Self {
            client,
            devices: HashMap::new(),
            folders: HashMap::new(),
            pending: HashMap::new(),
            since: 0,
        }
    }

    pub fn main_loop(&mut self) -> Result<()> {
        loop {
            self.get_events()?;
            self.print_status();
        }
    }

    fn get_events(&mut self) -> Result<()> {
        let response = self
            .client
            .get(&format!(
                "rest/events?since={}&events=FolderCompletion,DeviceDisconnected",
                self.since
            ))?
            .json::<EventsResponse>()?;

        let need_device_refresh = response
            .iter()
            .filter_map(|entry| match &entry.data {
                EventsResponseData::FolderCompletion { device, .. } => Some(device),
                _ => None,
            })
            .any(|item| !self.devices.contains_key(item));

        let need_folder_refresh = response
            .iter()
            .filter_map(|entry| match &entry.data {
                EventsResponseData::FolderCompletion { folder, .. } => Some(folder),
                _ => None,
            })
            .any(|item| !self.folders.contains_key(item));

        if need_device_refresh || need_folder_refresh {
            self.refresh_devices_and_folders()?;
        }

        response.iter().for_each(|entry| match &entry.data {
            EventsResponseData::FolderCompletion {
                device,
                folder,
                completion,
                ..
            } if *completion == ProgressPct(100.) => {
                self.pending.entry(device.clone()).and_modify(|v| {
                    v.remove(folder);
                });
            }
            EventsResponseData::FolderCompletion {
                device,
                folder,
                completion,
                need_bytes,
            } => {
                self.pending
                    .entry(device.clone())
                    .or_default()
                    .insert(folder.clone(), (*completion, *need_bytes));
            }

            EventsResponseData::DeviceDisconnected { id } => {
                self.pending.remove(id);
            }
        });

        self.since = response.last().map(|entry| entry.id).unwrap_or(self.since);

        self.refresh_connected_devices()?;

        Ok(())
    }

    fn refresh_connected_devices(&mut self) -> Result<()> {
        let response = self
            .client
            .get("rest/system/connections")?
            .json::<SystemConnectionsResponse>()?;

        response
            .connections
            .iter()
            .filter(|(_, v)| !v.connected)
            .for_each(|(id, _)| {
                self.pending.remove(id);
            });

        Ok(())
    }

    fn refresh_devices_and_folders(&mut self) -> Result<()> {
        log::debug!("Refreshing devices...");

        let response = self
            .client
            .get("rest/system/config")?
            .json::<SystemConfigResponse>()?;

        self.devices = response
            .devices
            .into_iter()
            .map(|entry| (entry.device_id, entry.name))
            .collect();

        self.folders = response
            .folders
            .into_iter()
            .map(|entry| (entry.id, entry.label))
            .collect();

        Ok(())
    }

    fn print_status(&self) {
        let text = self
            .pending
            .iter()
            .flat_map(|(_, folders)| {
                folders
                    .iter()
                    .map(|(_, (completion, need_bytes))| {
                        format!(" {}%/{}", completion, need_bytes)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join(" | ");

        let tooltip = self
            .pending
            .iter()
            .flat_map(|(device, folders)| {
                let device_name = self
                    .devices
                    .get(device)
                    .map(|v| v.as_str())
                    .unwrap_or(device.as_str());
                folders
                    .iter()
                    .map(|(folder, (completion, need_bytes))| {
                        let folder_name = self
                            .folders
                            .get(folder)
                            .map(|v| v.as_str())
                            .unwrap_or(folder.as_str());

                        format!(
                            "{:<10} {:<10} ({:.0}%, {})",
                            format!("{}:", device_name),
                            folder_name,
                            completion,
                            need_bytes
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        println!(
            "{}",
            serde_json::json!({
                "text": text,
                "tooltip": tooltip
            })
        );
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceID(String);

impl DeviceID {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceName(String);

impl DeviceName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
struct ProgressPct(f64);

impl fmt::Display for ProgressPct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.floor())
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NeedBytes(u64);

impl fmt::Display for NeedBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const BYTES_IN_MIB: u64 = 1024 * 1024;
        const BYTES_IN_GIB: u64 = 1024 * 1024 * 1024;

        let format_number = |value: f64| {
            if value.fract() == 0.0 {
                format!("{:.0}", value)
            } else {
                format!("{:.2}", value)
            }
        };

        if self.0 >= BYTES_IN_GIB {
            write!(
                f,
                "{} GiB",
                format_number(self.0 as f64 / BYTES_IN_GIB as f64)
            )
        } else {
            write!(
                f,
                "{} MiB",
                format_number(self.0 as f64 / BYTES_IN_MIB as f64)
            )
        }
    }
}

#[derive(Deserialize, Debug)]
struct SystemConnectionsResponse {
    connections: HashMap<DeviceID, SystemConnectionsResponseDevice>,
}

#[derive(Deserialize, Debug)]
struct SystemConnectionsResponseDevice {
    connected: bool,
}

#[derive(Deserialize, Debug)]
struct SystemConfigResponse {
    devices: Vec<SystemConfigResponseDevice>,
    folders: Vec<SystemConfigResponseFolder>,
}

#[derive(Deserialize, Debug)]
struct SystemConfigResponseDevice {
    #[serde(rename = "deviceID")]
    device_id: DeviceID,
    name: DeviceName,
}

#[derive(Deserialize, Debug)]
struct SystemConfigResponseFolder {
    id: FolderID,
    label: FolderName,
}

#[derive(Deserialize, Debug)]
enum EventsResponseType {
    FolderCompletion,
    DeviceDisconnected,
}

type EventsResponse = Vec<EventsResponseEntry>;

#[derive(Deserialize, Debug)]
struct EventsResponseEntry {
    id: u64,
    #[serde(flatten)]
    data: EventsResponseData,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct FolderID(String);

impl FolderID {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct FolderName(String);

impl FolderName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum EventsResponseData {
    DeviceDisconnected {
        id: DeviceID,
    },
    FolderCompletion {
        completion: ProgressPct,
        #[serde(rename = "needBytes")]
        need_bytes: NeedBytes,
        device: DeviceID,
        folder: FolderID,
    },
}
