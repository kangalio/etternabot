use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::serenity;

static CONFIG_PATH: &str = "config.json";
static DATA_PATH: &str = "data.json";

#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Quote {
	pub quote: String,
	pub source: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Config {
	pub minanyms: Vec<String>,
	pub quotes: Vec<Quote>,

	/// Channel where the MAX 300 role acquisition gratulations are posted
	pub promotion_gratulations_channel: serenity::ChannelId,
	/// Channel where bot watches and deletes messages without any links
	pub pack_releases_channel: serenity::ChannelId,
	/// Channel where bot watches and deletes messages without any files or links
	pub work_in_progress_channel: serenity::ChannelId,
	/// Channel that the bot redirects to in the above circumstances
	pub work_in_progress_discussion_channel: serenity::ChannelId,
	/// Channels in which bot commands can be used
	pub allowed_channels: Vec<serenity::ChannelId>,
	pub etterna_online_guild_id: serenity::GuildId,
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
	pub discord_id: serenity::UserId,
	pub discord_username: String,
	pub eo_id: u32,
	pub eo_username: String,
	pub last_known_num_scores: Option<u32>,
	pub last_rating: Option<etterna::Skillsets8>,
}

#[derive(Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct Data {
	pub user_registry: Vec<UserRegistryEntry>,
	rival_mapping: HashMap<serenity::UserId, String>, // discord username -> eo username
	preferred_scroll: HashMap<serenity::UserId, etterna::ScrollDirection>,
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

	pub fn set_scroll(&mut self, discord_user: serenity::UserId, scroll: etterna::ScrollDirection) {
		self.preferred_scroll.insert(discord_user, scroll);
	}

	pub fn scroll(&self, discord_user: serenity::UserId) -> Option<etterna::ScrollDirection> {
		self.preferred_scroll.get(&discord_user).copied()
	}

	pub fn set_rival(&mut self, discord_user: serenity::UserId, rival: String) -> Option<String> {
		self.rival_mapping.insert(discord_user, rival)
	}

	pub fn rival(&self, discord_user: serenity::UserId) -> Option<&str> {
		self.rival_mapping.get(&discord_user).map(|x| x as _)
	}
}
