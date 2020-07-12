mod pattern_visualize;

use crate::serenity; // use my custom serenity prelude
use crate::api::*;

const BOT_PREFIX: &str = "+";

pub struct Handler {
	session: std::sync::Mutex<crate::api::Session>,
}

impl Handler {
	pub fn from_session(session: Session) -> Self {
		Self { session: std::sync::Mutex::new(session) }
	}

	fn command(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		text: &str
	) -> Result<(), Box<dyn std::error::Error>> {
		let mut session = self.session.lock().unwrap();

		match cmd {
			"ping" => {
				msg.channel_id.say(&ctx.http, "Pong!")?;
			},
			"user" => {
				let reply = match session.user_details(text) {
					Ok(user) => format!("{} {}", user.username, user.player_rating),
					Err(Error::UserNotFound) => format!("User '{}' was not found", text),
					Err(other) => format!("{:?}", other),
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

		let mut a = text.splitn(2, ' ');
		let command_name = a.next().unwrap().trim();
		let parameters = a.next().unwrap_or("").trim();

		if let Err(e) = self.command(&ctx, &msg, command_name, parameters) {
			if let Err(inner_e) = msg.channel_id.say(&ctx.http, "") {
				println!("Failed with '{}' while sending error message '{}'", inner_e, e);
			}
		}
	}
}