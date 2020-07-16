use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};

static CONFIG_PATH: &str = "config.json";

#[derive(Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Config {
	discord_eo_username_mapping: HashMap<String, String>,
}

impl Config {
	pub fn load() -> Self {
		let config_path = Path::new(CONFIG_PATH);
		if config_path.exists() {
			let config_contents = std::fs::read_to_string(config_path)
				.expect("Couldn't read config JSON file");
			
			serde_json::from_str(&config_contents)
				.expect("Config JSON had invalid format")
		} else {
			std::default::Default::default()
		}
	}

	pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
		serde_json::to_writer_pretty(std::fs::File::create(CONFIG_PATH)?, self)?;
		Ok(())
	}

	// Returns the old EO username, if there was one registered
	pub fn set_eo_username(&mut self, discord_username: &str, eo_username: &str) -> Option<String> {
		// who needs allocation efficiency lul
		self.discord_eo_username_mapping.insert(
			discord_username.to_owned(),
			eo_username.to_owned(),
		)

	}

	// we need String here because the string can come either from `self` or from the passed
	// parameter. So we have differing lifetimes which we can't encode with a `&str`
	pub fn eo_username(&self, discord_username: &str) -> String {
		match self.discord_eo_username_mapping.get(discord_username) {
			Some(username) => username.to_owned(),
			None => discord_username.to_owned(),
		}
	}
}