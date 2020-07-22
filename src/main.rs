#[allow(clippy::len_zero, clippy::tabs_in_doc_comments)]

mod discord_handler;
mod auth;
#[allow(unused)]
mod wife;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		prelude::*,
		model::{gateway::Ready, channel::Message, id::{UserId, ChannelId}, guild::Member},
		framework::standard::{Args, Delimiter},
		http::error::{ErrorResponse, Error as HttpError, DiscordJsonError},
		utils::Colour as Color,
		Error,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

fn main() -> Result<(), Box<dyn std::error::Error>> {
	struct Handler {
		state: std::sync::Mutex<discord_handler::State>,
	}

	impl serenity::EventHandler for Handler {
		fn ready(&self, _: serenity::Context, ready: serenity::Ready) {
			println!("Connected to Discord as {}", ready.user.name);
		}
	
		fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
			// hehe no, we don't want endless message chains
			// (originally I wanted to just ignore own messages, but that's awkward to implement so
			// let's just ignore all bot messages)
			if msg.author.bot { return }

			let mut state = match self.state.lock() {
				Ok(a) => a,
				Err(std::sync::PoisonError { .. }) => std::process::exit(1),
			};
			if let Err(e) = state.message(&ctx, &msg).map_err(|e| {
				// this looks complicated, but all it does is map serenity's confusing
				// "[Serenity] No correct json was received!" error to one of my (more descriptive)
				// error types
				if let discord_handler::Error::SerenityError(serenity::Error::Http(ref e)) = e {
					if let serenity::HttpError::UnsuccessfulRequest(serenity::ErrorResponse {
						error: serenity::DiscordJsonError { code: -1, .. },
						..
					}) = **e {
						return discord_handler::Error::AttemptedToSendInvalidMessage;
					}
				}
				e
			}) {
				println!("Error {:?}", e);
				let error_msg = e.to_string();
				if let Err(inner_e) = msg.channel_id.say(&ctx.http, &error_msg) {
					println!("Failed with '{:?}' while sending error message '{}'", inner_e, &error_msg);
				}
			}
		}

		fn guild_member_update(&self,
			ctx: serenity::Context,
			old: Option<serenity::Member>,
			new: serenity::Member
		) {
			let mut state = self.state.lock().unwrap();
			if let Err(e) = state.guild_member_update(ctx, old, new) {
				println!("Error in guild member update: {:?}", e);
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
