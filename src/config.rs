//! Configuration file and settings management
//! Modified from nostr-rs-relay
//!
//! The MIT License (MIT)
//! Copyright (c) 2021 Greg Heartsfield
/*
 The MIT License (MIT)
 Copyright (c) 2021 Greg Heartsfield

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/

use std::path::PathBuf;
use std::str::FromStr;

use cdk::Amount;
use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Info {
    pub url: String,
    #[serde(default = "path_default")]
    pub db_path: PathBuf,
    #[serde(default = "last_pay_path")]
    pub last_pay_path: String,
    pub listen_host: String,
    pub listen_port: u16,
    pub mnemonic: String,
    #[serde(default = "derivation_path_default")]
    pub derivation_path: String,
    #[serde(default = "max_order_default")]
    pub max_order: u8,
    pub min_fee_reserve: Amount,
    pub min_fee_percent: f32,
}

fn path_default() -> PathBuf {
    PathBuf::from_str("/tmp/config-rs-mint/cashu-rs-mint.redb").unwrap()
}

fn derivation_path_default() -> String {
    "0/0/0/0".to_string()
}

fn max_order_default() -> u8 {
    32
}

fn last_pay_path() -> String {
    "/tmp/config-rs-mint/last_path".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum LnBackend {
    #[default]
    Cln,
    Greenlight,
    Ldk,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ln {
    pub ln_backend: LnBackend,
    pub cln_path: Option<PathBuf>,
    pub greenlight_invite_code: Option<String>,
    pub invoice_description: Option<String>,
    pub fee_percent: f64,
    pub reserve_fee_min: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub info: Info,
    //    pub mint_info: MintInfo,
    pub ln: Ln,
}

impl Settings {
    #[must_use]
    pub fn new(config_file_name: &Option<String>) -> Self {
        let default_settings = Self::default();
        // attempt to construct settings with file
        let from_file = Self::new_from_default(&default_settings, config_file_name);
        match from_file {
            Ok(f) => f,
            Err(e) => {
                warn!("Error reading config file ({:?})", e);
                default_settings
            }
        }
    }

    fn new_from_default(
        default: &Settings,
        config_file_name: &Option<String>,
    ) -> Result<Self, ConfigError> {
        let mut default_config_file_name = dirs::config_dir()
            .ok_or(ConfigError::NotFound("Config Path".to_string()))?
            .join("cashu-rs-mint");

        default_config_file_name.push("config.toml");
        let config: String = match config_file_name {
            Some(value) => value.clone(),
            None => default_config_file_name.to_string_lossy().to_string(),
        };
        let builder = Config::builder();
        let config: Config = builder
            // use defaults
            .add_source(Config::try_from(default)?)
            // override with file contents
            .add_source(File::with_name(&config))
            .build()?;
        let settings: Settings = config.try_deserialize()?;

        debug!("{settings:?}");

        match settings.ln.ln_backend {
            LnBackend::Cln => assert!(settings.ln.cln_path.is_some()),
            LnBackend::Greenlight => (),
            LnBackend::Ldk => (),
        }

        Ok(settings)
    }
}
