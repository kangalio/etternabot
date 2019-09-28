extern crate reqwest;
mod structs;
mod config;

pub fn login() -> Result<String, Error> {
    let resp: structs::Login = reqwest::Client::new()
        .post("https://api.etternaonline.com/v2/login")
        .form(&[
            ("username", &config::username),
            ("password", &config::password),
            ("clientData", &config::client_data)
        ])
        .send()?
        .json()?;
    Ok(
        match resp.data {
            Some(data) => data.attributes.accessToken.to_string(),
            None => String::new()
        }
    )
}

fn _get(key: &str, path: &str) -> Result<reqwest::Response, reqwest::Error> {
    reqwest::Client::new()
        .get(&format!("https://api.etternaonline.com/v2/{}", path))
        .bearer_auth(key)
        .send()
}

pub fn get_user(key: &str, username: &str) -> Result<structs::User, Error> {
    let data: structs::UserData = _get(key, &format!("user/{}", username))?.json()?;
    match data.data {
        Some(user) => Ok(user),
        None => Err(Error::from("User not found"))
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