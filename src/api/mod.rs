use reqwest::blocking as req;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("user not found")]
	UserNotFound,
	#[error("username and password combination not found")]
	InvalidLogin,
	#[error("server returned a response code that was not expected")]
	UnexpectedResponseCode(u16),
	#[error("network request failed")]
	Reqwest(#[from] reqwest::Error),
}

pub struct User {
	pub username: String,
	pub about_me: String,
	pub is_moderator: bool,
	pub is_patreon: bool,
	pub avatar_url: String,
	pub country_code: String,
	pub player_rating: f64,
	pub default_modifiers: String,
	pub skillsets: Skillsets,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Skillsets {
	pub stream: f64,
	pub jumpstream: f64,
	pub handstream: f64,
	pub stamina: f64,
	pub jackspeed: f64,
	pub chordjack: f64,
	pub technical: f64
}

pub struct Session {
	// This stuff is needed for re-login
	username: String,
	password: String,
	client_data: String,

	key: String,
	client: req::Client,
	// TODO: Implement arbitrary rate limit
}

impl Session {
	pub fn new_from_login(
		username: String,
		password: String,
		client_data: String,
	) -> Result<Self, Error> {
		let client = req::Client::new();
		let key = "dummy key that will be replaced anyway when I login".into();

		let mut session = Self { username, password, client_data, key, client };
		session.login()?;

		Ok(session)
	}

	pub fn login(&mut self) -> Result<(), Error> {
		let response = self.client
			.post("https://api.etternaonline.com/v2/login")
			.form(&[
				("username", &self.username),
				("password", &self.password),
				("clientData", &self.client_data)
			])
			.send()?;
			
		match response.status().as_u16() {
			404 => Err(Error::InvalidLogin),
			200 => {
				let json: serde_json::Value = response.json()?;
				let key = json["data"]["attributes"]["accessToken"].as_str()
					.expect("Received an access token that is not a string");
				self.key = key.to_owned();
				Ok(())
			}
			other => panic!("Unexpected response code {}", other),
		}
	}

	fn get(&mut self,
		builder: impl Fn(&req::Client) -> req::RequestBuilder
	) -> Result<(u16, serde_json::Value), Error> {
		let response = builder(&self.client)
			.bearer_auth(&self.key)
			.send()?;
		
		if response.status().as_u16() == 401 {
			// Token expired, let's login again and retry
			self.login()?;
			self.get(builder)
		} else {
			Ok((response.status().as_u16(), response.json()?))
		}
	}

	pub fn user_details(&mut self, username: &str) -> Result<User, Error> {
		let json = match self.get(|client| {
			client.get(&format!("https://api.etternaonline.com/v2/user/{}", username))
		})? {
			(200, json) => json,
			(404, _) => return Err(Error::UserNotFound),
			(code, _) => return Err(Error::UnexpectedResponseCode(code)),
		};

		let json = &json["data"]["attributes"];

		Ok(User {
			username: json["userName"].as_str().unwrap().to_owned(),
			about_me: json["aboutMe"].as_str().unwrap().to_owned(),
			is_moderator: json["moderator"].as_bool().unwrap(),
			is_patreon: json["patreon"].as_bool().unwrap(),
			avatar_url: json["avatar"].as_str().unwrap().to_owned(),
			country_code: json["countryCode"].as_str().unwrap().to_owned(),
			player_rating: json["playerRating"].as_f64().unwrap(),
			default_modifiers: json["defaultModifiers"].as_str().unwrap().to_owned(),
			skillsets: Skillsets {
				stream: json["skillsets"]["Stream"].as_f64().unwrap(),
				jumpstream: json["skillsets"]["Jumpstream"].as_f64().unwrap(),
				handstream: json["skillsets"]["Handstream"].as_f64().unwrap(),
				stamina: json["skillsets"]["Stamina"].as_f64().unwrap(),
				jackspeed: json["skillsets"]["JackSpeed"].as_f64().unwrap(),
				chordjack: json["skillsets"]["Chordjack"].as_f64().unwrap(),
				technical: json["skillsets"]["Technical"].as_f64().unwrap(),
			}
		})
	}
}