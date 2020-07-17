use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};

static CONFIG_PATH: &str = "config.json";

#[derive(Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Config {
	#[serde(default)]
	discord_eo_username_mapping: HashMap<String, String>,
	#[serde(default)]
	rival_mapping: HashMap<String, String>, // discord username -> eo username
	#[serde(default)]
	preferred_scroll: HashMap<String, super::pattern_visualize::ScrollType>,
	
	minanyms: Vec<String>,
	#[serde(default)]
	minanym_index: usize,
}

impl Config {
	pub fn load() -> Self {
		let config_path = Path::new(CONFIG_PATH);
		let config: Self = if config_path.exists() {
			let config_contents = std::fs::read_to_string(config_path)
				.expect("Couldn't read config JSON file");
			
			serde_json::from_str(&config_contents)
				.expect("Config JSON had invalid format")
		} else {
			std::default::Default::default()
		};

		if config.minanyms.is_empty() {
			panic!("Empty minanyms!");
		}
		
		config
	}

	pub fn save(&self) -> anyhow::Result<()> {
		serde_json::to_writer_pretty(std::fs::File::create(CONFIG_PATH)?, self)?;
		Ok(())
	}

	// Returns the old EO username, if there was one registered
	pub fn set_eo_username(&mut self, discord_username: String, eo_username: String) -> Option<String> {
		self.discord_eo_username_mapping.insert(discord_username, eo_username)

	}

	// we need String here because the string can come either from `self` or from the passed
	// parameter. So we have differing lifetimes which we can't encode with a `&str`
	pub fn eo_username(&self, discord_username: &str) -> Option<&str> {
		self.discord_eo_username_mapping.get(discord_username).map(|s| s as _)
	}

	pub fn set_scroll(&mut self, discord_username: String, scroll: super::pattern_visualize::ScrollType) {
		self.preferred_scroll.insert(discord_username, scroll);
	}

	pub fn scroll(&self, discord_username: &str) -> Option<super::pattern_visualize::ScrollType> {
		self.preferred_scroll.get(discord_username).copied()
	}

	pub fn set_rival(&mut self, discord_username: String, rival: String) -> Option<String> {
		self.rival_mapping.insert(discord_username, rival)
	}

	pub fn rival(&self, discord_username: &str) -> Option<&str> {
		self.rival_mapping.get(discord_username).map(|x| x as _)
	}

	pub fn make_description(&mut self) -> String {
		let description = format!(
			"
Here are my commands: (Descriptions by Fission)

**+profile [username]**
*Show your fabulously superberful profile*
**+top10 [username] [skillset]**
*For when top9 isn't enough*
**+top[nn] [username] [skillset]**
*Sometimes we take things too far*
**+compare [user1] [user2]**
*One person is an objectively better person than the other, find out which one!*
**+rival**
*But are you an objectively better person than gary oak?*
**+rivalset [username]**
*Replace gary oak with a more suitable rival*
**+userset [username]**
*Don't you dare set your user to* {} *you imposter*

More commands:
**+pattern [down/up] [pattern string]**
*Visualize note patterns, for example* `[14][23][14]` *or* `4321234`
**+scrollset [down/up]**
*Set your preferred scroll type that will be used as a default*
**+lastsession [username]**
*Show the last 10 scores*
**+help**
*Print this message*

You can also post links to scores and songs and I will show info about them
			",
			self.minanyms[self.minanym_index]
		);

		self.minanym_index = (self.minanym_index + 1) % self.minanyms.len();

		description
	}
}