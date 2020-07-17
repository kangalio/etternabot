#[allow(clippy::len_zero, clippy::tabs_in_doc_comments)]

mod discord_handler;
mod auth;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		prelude::*,
		model::{gateway::Ready, channel::Message, id::{UserId, ChannelId}},
		framework::standard::{Args, Delimiter},
		utils::Colour as Color,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

fn main() -> anyhow::Result<()> {
	struct Handler {
		state: std::sync::Mutex<discord_handler::State>,
	}

	impl serenity::EventHandler for Handler {
		fn ready(&self, _: serenity::Context, ready: serenity::Ready) {
			println!("Connected to Discord as {}", ready.user.name);
		}
	
		fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
			// hehe no, we don't want endless message chains
			// (originally I wanted to just ignore own messages, but that's awkward so let's just
			// ignore all bot messages)
			if msg.author.bot { return }

			let mut state = self.state.lock().unwrap();
			if let Err(e) = state.message(&ctx, &msg) {
				let error_msg = e.to_string();
				if !error_msg.contains("don't print this") {
					if let Err(inner_e) = msg.channel_id.say(&ctx.http, &error_msg) {
						println!("Failed with '{}' while sending error message '{}'", inner_e, &error_msg);
					}
				}
			}
		}
	}

	let handler = Handler { state: std::sync::Mutex::new(discord_handler::State::load()?) };
	println!("Logged into EO");

	// Login to Discord and start bot
	let mut client = serenity::Client::new(auth::DISCORD_BOT_TOKEN, handler)
		.expect("Unable to create Discord client");
	client.start()?;

	Ok(())
}
