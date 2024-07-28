pub mod model;
pub use model::*;

#[derive(Debug)]
pub enum Error {
	Http(reqwest::Error),
}

impl From<reqwest::Error> for Error {
	fn from(error: reqwest::Error) -> Self {
		Self::Http(error)
	}
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Http(e) => write!(f, "network error: {}", e),
		}
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Http(e) => Some(e),
		}
	}
}

#[derive(serde::Deserialize)]
struct Response<T> {
	data: T,
}

#[derive(Default)]
pub enum ScoresOrdering {
	#[default]
	DatetimeAscending,
	DatetimeDescending,
}

#[derive(Default)]
pub struct ScoresRequest {
	pub limit: Option<u32>,
	pub include_invalid: bool,
	pub ordering: ScoresOrdering,
}

pub struct Client {
	reqwest: reqwest::Client,
}

impl Client {
	pub fn new() -> Self {
		Self {
			reqwest: reqwest::Client::new(),
		}
	}

	pub async fn scores(
		&self,
		username: &str,
		request: ScoresRequest,
	) -> Result<Vec<Score>, Error> {
		let url =
			format!(
			"https://api.etternaonline.com/api/users/{}/scores?limit={}&sort={}&filter[valid]={}",
			username,
			match request.limit {
				Some(n) => n.to_string(),
				None => "-1".to_string(), // "" and "0" don't do the trick
			},
			match request.ordering {
				ScoresOrdering::DatetimeAscending => "datetime",
				ScoresOrdering::DatetimeDescending => "-datetime",
			},
			if request.include_invalid { "false" } else { "true" },
		);

		Ok(self
			.reqwest
			.get(url)
			.send()
			.await?
			.json::<Response<Vec<Score>>>()
			.await?
			.data)
	}

	pub async fn user(&self, username: &str) -> Result<User, Error> {
		let url = format!("https://api.etternaonline.com/api/users/{}", username);
		Ok(self
			.reqwest
			.get(url)
			.send()
			.await?
			.json::<Response<User>>()
			.await?
			.data)
	}
}
