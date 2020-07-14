mod pattern_visualize;

use crate::serenity; // use my custom serenity prelude
use etternaonline_api as eo;

const BOT_PREFIX: &str = "+";

const CMD_TOP_HELP: &str = "Call this command with `+topNN [USERNAME] [SKILLSET]` (both params optional)";

struct Config {

}

impl Config {
	fn load() -> Self {
		Self { }
	}

	// we need String here because the string can come either from `self` or from the passed
	// parameter. So we have differing lifetimes which we can't encode with a `&str`
	fn eo_username(&self, discord_username: &str) -> String {
		discord_username.to_owned() // STUB
	}
}

struct State {
	config: Config,
	session: eo::Session,
}

pub struct Handler {
	state: std::sync::Mutex<State>,
}

impl Handler {
	pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
		let state = State {
			session: eo::Session::new_from_login(
				crate::auth::EO_USERNAME.to_owned(),
				crate::auth::EO_PASSWORD.to_owned(),
				crate::auth::EO_CLIENT_DATA.to_owned(),
				std::time::Duration::from_millis(1000),
				Some(std::time::Duration::from_millis(5000)),
			)?,
			config: Config::load(),
		};

		Ok(Self { state: std::sync::Mutex::new(state) })
	}

	fn top_scores(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		mut state: impl std::ops::DerefMut<Target=State>,
		text: &str,
		mut limit: u32,
	) -> Result<(), Box<dyn std::error::Error>> {
		let args: Vec<&str> = text.split_whitespace().collect();

		let skillset;
		let eo_username;
		if args.len() == 0 {
			skillset = None;
			eo_username = state.config.eo_username(&msg.author.name);
		} else if args.len() == 1 {
			match eo::Skillset::from_user_input(args[0]) {
				Some(parsed_skillset) => {
					skillset = Some(parsed_skillset);
					eo_username = state.config.eo_username(&msg.author.name);
				},
				None => {
					skillset = None;
					eo_username = args[0].to_owned(); // to_owned not strictly needed
				},
			}
		} else if args.len() == 2 {
			skillset = match eo::Skillset::from_user_input(args[0]) {
				Some(parsed_skillset) => Some(parsed_skillset),
				None => {
					msg.channel_id.say(
						&ctx.http,
						format!("Unrecognized skillset \"{}\"", args[0]))?;
					return Ok(());
				}
			};
			eo_username = args[1].to_owned(); // to_owned not strictly needed
		} else {
			msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			return Ok(());
		}

		// Download top scores
		let top_scores = match skillset {
			None => state.session.user_top_10_scores(&eo_username),
			Some(skillset) => state.session.user_top_skillset_scores(&eo_username, skillset, limit),
		};
		if let Err(eo::Error::UserNotFound) = top_scores {
			msg.channel_id.say(&ctx.http, format!("No such user \"{}\"", eo_username))?;
			return Ok(());
		}
		let top_scores = top_scores?;

		let country_code = state.session.user_details(&eo_username)?.country_code;

		let mut response = String::from("```");
		for (i, entry) in top_scores.iter().enumerate() {
			response += &format!(
				"{}. {}: {:.2}x\n  ▸ Score: {:.2} Wife: {:.2}%\n",
				i + 1,
				&entry.song_name,
				entry.rate,
				entry.ssr_overall,
				entry.wifescore * 100.0,
			);
		}

		if limit != 10 && skillset == None {
			limit = 10;
			response += "(due to a bug in the EO v2 API, only 10 entries can be shown)";
		}
		
		response += "```";

		let title = match skillset {
			None => format!("{}'s Top {}", eo_username, limit),
			Some(skillset) => format!("{}'s Top {} {}", eo_username, limit, skillset),
		};

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
			.color(crate::ETTERNA_COLOR)
			.description(&response)
			.author(|a| a
				.name(title)
				.url(format!("https://etternaonline.com/user/profile/{}", eo_username))
				.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", country_code))
			)
		))?;

		Ok(())
	}

	fn command(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		text: &str
	) -> Result<(), Box<dyn std::error::Error>> {
		let mut state = self.state.lock().unwrap();

		if cmd.starts_with("top") {
			if let Ok(limit @ 1..=100) = cmd[3..].parse() {
				self.top_scores(ctx, msg, state, text, limit)?;
			} else {
				msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			}
			return Ok(());
		}

		match cmd {
			"ping" => {
				msg.channel_id.say(&ctx.http, "Pong!")?;
			},
			"user" => {
				let eo_username = if text.is_empty() {
					Some(state.config.eo_username(&msg.author.name))
				} else {
					None
				};
				let eo_username = eo_username.as_deref().unwrap_or(text);

				let reply = match state.session.user_details(&eo_username) {
					Ok(user) => format!("{} {}", user.username, user.player_rating),
					Err(eo::Error::UserNotFound) => format!("User '{}' was not found", eo_username), // TODO: add "maybe you need to add your EO username" msg here
					Err(other) => format!("An error occurred ({})", other),
				};
				msg.channel_id.say(&ctx.http, &reply)?;
			},
			"pattern" => {
				pattern_visualize::generate("noteskin.png", "output.png", text)?;

				// Send the image into the channel where the summoning message comes from
				msg.channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;
			}
			_ => {},
		}
		Ok(())
	}
}

impl serenity::EventHandler for Handler {
	fn ready(&self, _: serenity::Context, ready: serenity::Ready) {
		println!("Connected to Discord as {}", ready.user.name);
	}

	fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
		if !msg.content.starts_with(BOT_PREFIX) { return }
		let text = &msg.content[BOT_PREFIX.len()..];

		// Let's not do this, because if a non existing command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		// Split message into command part and parameter part
		let mut a = text.splitn(2, ' ');
		let command_name = a.next().unwrap().trim();
		let parameters = a.next().unwrap_or("").trim();

		if let Err(e) = self.command(&ctx, &msg, command_name, parameters) {
			if let Err(inner_e) = msg.channel_id.say(&ctx.http, format!("{}", e)) {
				println!("Failed with '{}' while sending error message '{}'", inner_e, e);
			}
		}
	}
}