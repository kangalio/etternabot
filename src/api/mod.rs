#![allow(non_snake_case)] // eh

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
      Some(data) => data.attributes.accessToken,
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
  pub userName: String,
  pub aboutMe: String,
  pub moderator: bool,
  pub patreon: bool,
  pub avatar: String,
  pub countryCode: String,
  pub playerRating: f64,
  pub defaultModifiers: String,
  pub skillsets: Skillsets
}

#[derive(Deserialize, Debug)]
pub struct Skillsets {
  pub Stream: f64,
  pub Jumpstream: f64,
  pub Handstream: f64,
  pub Stamina: f64,
  pub JackSpeed: f64,
  pub Chordjack: f64,
  pub Technical: f64
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
  pub r#type: String,
  pub id: String,
  pub attributes: LoginAttributes,
}

#[derive(Deserialize, Debug)]
pub struct LoginAttributes {
  pub accessToken: String,
  pub expiresAt: i32
}