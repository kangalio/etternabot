use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

static CONFIG_PATH: &str = "config.json";
static DATA_PATH: &str = "data.json";

#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Quote {
	pub quote: String,
	pub source: Option<String>,
}

#[derive(Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Config {
	pub minanyms: Vec<String>,
	pub quotes: Vec<Quote>,

	/// Channel where the MAX 300 role acquisition gratulations are posted
	pub promotion_gratulations_channel: u64,
	/// Channel where bot watches and deletes messages without any links
	pub pack_releases_channel: u64,
	/// Channel where bot watches and deletes messages without any files or links
	pub work_in_progress_channel: u64,
	/// Channel that the bot redirects to in the above circumstances
	pub work_in_progress_discussion_channel: u64,
	/// Channels in which bot commands can be used
	pub allowed_channels: Vec<u64>,
	/// Channel to scan for score screenshots in
	pub score_channel: u64,
	/// Channel to post the requested score cards into
	pub score_ocr_card_channel: u64,
	pub etterna_online_guild_id: u64,
	// Only these people's images in `score_channel` will be used
	pub score_ocr_allowed_eo_role: u64,
}

impl Config {
	pub fn load() -> Self {
		let config_path = Path::new(CONFIG_PATH);
		let config_contents =
			std::fs::read_to_string(config_path).expect("Couldn't read config JSON file");

		let config: Self =
			serde_json::from_str(&config_contents).expect("Config JSON had invalid format");

		if config.minanyms.is_empty() {
			panic!("Empty minanyms!");
		}

		config
	}
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct UserRegistryEntry {
	pub discord_id: u64,
	pub discord_username: String,
	pub eo_id: u32,
	pub eo_username: String,
	pub last_known_num_scores: Option<u32>,
	pub last_rating: Option<etterna::Skillsets8>,
}

#[derive(Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct Data {
	pub user_registry: Vec<UserRegistryEntry>,
	rival_mapping: HashMap<u64, String>, // discord username -> eo username
	preferred_scroll: HashMap<u64, etterna::ScrollDirection>,
}

impl Data {
	pub fn load() -> Self {
		let data_path = Path::new(DATA_PATH);
		let data: Self = if data_path.exists() {
			let config_contents =
				std::fs::read_to_string(data_path).expect("Couldn't read data JSON file");

			serde_json::from_str(&config_contents).expect("Data JSON had invalid format")
		} else {
			Default::default()
		};

		data
	}

	pub fn save(&self) {
		serde_json::to_writer_pretty(
			std::fs::File::create(DATA_PATH).expect("Couldn't write to data json file"),
			self,
		)
		.expect("Couldn't deserialize data into a json");
	}

	pub fn set_scroll(&mut self, discord_user: u64, scroll: etterna::ScrollDirection) {
		self.preferred_scroll.insert(discord_user, scroll);
	}

	pub fn scroll(&self, discord_user: u64) -> Option<etterna::ScrollDirection> {
		self.preferred_scroll.get(&discord_user).copied()
	}

	pub fn set_rival(&mut self, discord_user: u64, rival: String) -> Option<String> {
		self.rival_mapping.insert(discord_user, rival)
	}

	pub fn rival(&self, discord_user: u64) -> Option<&str> {
		self.rival_mapping.get(&discord_user).map(|x| x as _)
	}
}
