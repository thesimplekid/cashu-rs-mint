use std::fmt::{Display, Formatter};
use std::str::FromStr;

use bech32::{ToBase32, Variant};
use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use super::error::Error;

#[derive(Debug, Clone)]
pub struct LnUrl {
    url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "login")]
    Login,
    #[serde(rename = "register")]
    Register,
    #[serde(rename = "link")]
    Link,
    #[serde(rename = "auth")]
    Auth,
}

impl ToString for Action {
    fn to_string(&self) -> String {
        match self {
            Action::Login => "login".to_string(),
            Action::Register => "register".to_string(),
            Action::Link => "link".to_string(),
            Action::Auth => "auth".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponseParams {
    pub tag: Option<Action>,
    pub k1: String,
    pub sig: String,
    pub key: String,
    pub jwt: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenResponse {
    pub status: String,
    pub token: String,
}

impl LnUrl {
    pub fn _new_auth_lnurl(url: Url, tag: Action, action: Action) -> Self {
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 32] = rng.gen();

        let k1 = hex::encode(random_bytes);

        let url = format!(
            "{}?tag={}&k1={}&action={}",
            url,
            tag.to_string(),
            k1,
            action.to_string()
        );

        Self { url }
    }

    pub fn encode(&self) -> String {
        let base32 = self.url.as_bytes().to_base32();
        bech32::encode("lnurl", base32, Variant::Bech32).unwrap()
    }

    pub fn _decode(lnurl: String) -> Result<LnUrl, Error> {
        LnUrl::from_str(&lnurl)
    }
}

impl Display for LnUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl Serialize for LnUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for LnUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        LnUrl::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for LnUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if s.to_lowercase().starts_with("lnurl") {
            let (_, data, _) = bech32::decode(s).map_err(|_| Error::InvalidLnUrl)?;
            let bytes = bech32::FromBase32::from_base32(&data).map_err(|_| Error::InvalidLnUrl)?;
            let url = String::from_utf8(bytes).map_err(|_| Error::InvalidLnUrl)?;
            Ok(LnUrl { url })
        } else {
            Err(Error::InvalidLnUrl)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_lnurl_test() {
        let service = url::Url::from_str("https://service.com").unwrap();
        let lnurl = LnUrl::_new_auth_lnurl(service, Action::Login, Action::Login);

        println!("{}", lnurl.encode());

        panic!()
    }
}

/*
Modified From lnurl-rs
https://github.com/benthecarman/lnurl-rs

MIT License

Copyright (c) 2023 benthecarman

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/
