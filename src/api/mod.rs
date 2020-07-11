// Authentication data, i.e. passwords and stuff are saved there
mod config;

use reqwest::blocking;
use reqwest::blocking::Response;
use serde::Deserialize;


pub struct Session {
  key: String,
  // TODO: Implement arbitrary rate limit
}

impl Session {
  pub fn login() -> Result<Self, Error> {
    let resp: Login = blocking::Client::new()
    .post("https://api.etternaonline.com/v2/login")
    .form(&[
      ("username", &config::username),
      ("password", &config::password),
      ("clientData", &config::client_data)
    ])
    .send()?
    .json()?;
    
    let key = match resp.data {
      Some(data) => data.attributes.access_token,
      None => return Err(Error::from("Incorrect login data, probably.")),
    };

    Ok(Self { key })
  }

  fn get(&self, path: &str) -> Result<Response, reqwest::Error> {
    blocking::Client::new()
      .get(&format!("https://api.etternaonline.com/v2/{}", path))
      .bearer_auth(&self.key)
      .send()
  }

  pub fn get_user(&self, username: &str) -> Result<User, Error> {
    let data: UserData = self.get(&format!("user/{}", username))?.json()?;
    match data.data {
      Some(user) => Ok(user),
      None => Err(Error::from("User not found"))
    }
  }
}

#[derive(Debug)]
pub enum Error {
  ApiError(String),
  Reqwest(reqwest::Error)
}

impl From<reqwest::Error> for Error {
  #[inline]
  fn from(err: reqwest::Error) -> Error {
    Error::Reqwest(err)
  }
}

impl From<&str> for Error {
  #[inline]
  fn from(err: &str) -> Error {
    Error::ApiError(err.to_string())
  }
}

#[derive(Deserialize, Debug)]
pub struct UserData {
  pub data: Option<User>,
  pub errors: Option<Vec<EOError>>
}

#[derive(Deserialize, Debug)]
pub struct User {
  pub r#type: String,
  pub id: String,
  pub attributes: Attributes
}

#[derive(Deserialize, Debug)]
pub struct Attributes {
  #[serde(rename = "userName")]
  pub user_name: String,
  #[serde(rename = "aboutMe")]
  pub about_me: String,
  pub moderator: bool,
  pub patreon: bool,
  pub avatar: String,
  #[serde(rename = "countryCode")]
  pub country_code: String,
  #[serde(rename = "playerRating")]
  pub player_rating: f64,
  #[serde(rename = "defaultModifiers")]
  pub default_modifiers: String,
  pub skillsets: Skillsets
}

#[derive(Deserialize, Debug)]
pub struct Skillsets {
  #[serde(rename = "Stream")]
  pub stream: f64,
  #[serde(rename = "Jumpstream")]
  pub jumpstream: f64,
  #[serde(rename = "Handstream")]
  pub handstream: f64,
  #[serde(rename = "Stamina")]
  pub stamina: f64,
  #[serde(rename = "JackSpeed")]
  pub jackspeed: f64,
  #[serde(rename = "Chordjack")]
  pub chordjack: f64,
  #[serde(rename = "Technical")]
  pub technical: f64
}

#[derive(Deserialize, Debug)]
pub struct Login {
  pub data: Option<LoginData>,
  pub errors: Option<Vec<EOError>>
}

#[derive(Deserialize, Debug)]
pub struct EOError {
  pub status: u32,
  pub title: String,
  pub detail: String
}

#[derive(Deserialize, Debug)]
pub struct LoginData {
  #[serde(rename = "id")]
  pub user_id: String,
  pub attributes: LoginAttributes,
}

#[derive(Deserialize, Debug)]
pub struct LoginAttributes {
  #[serde(rename = "accessToken")]
  pub access_token: String,
  #[serde(rename = "expiresAt")]
  pub expires_at: i32
}