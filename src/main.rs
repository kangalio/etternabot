#![allow(clippy::len_zero, clippy::tabs_in_doc_comments, clippy::collapsible_if, clippy::needless_bool)]
#![warn(clippy::indexing_slicing)]

mod discord_handler;
mod auth;
mod mutex;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		prelude::*,
		model::{
			user::User,
			gateway::Ready,
			channel::{Message, Reaction, ReactionType},
			id::{UserId, ChannelId, MessageId, GuildId},
			guild::Member
		},
		http::error::{ErrorResponse, Error as HttpError, DiscordJsonError},
		utils::Colour as Color,
		Error,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

fn main() -> Result<(), Box<dyn std::error::Error>> {
	macro_rules! lock {
		($this:ident, $state:ident) => {
			let mut guard;
			let $state = loop {
				// if poisened, kill the whole process instead of failing over and over again
				guard = $this.state.read().unwrap_or_else(|_| {
					println!("RwLock locking failed! TERMINATING THE PROGRAM!");
					std::process::exit(1);
				});
	
				match &*guard {
					Some(state) => break state,
					None => {
						drop(guard); // important! or the login attempt can't finish
						// if the bot is not ready yet, wait a bit and check again
						std::thread::sleep(std::time::Duration::from_millis(100));
					}
				};
			};
		}
	}

	struct Handler {
		state: std::sync::RwLock<Option<discord_handler::State>>,
	}

	impl serenity::EventHandler for Handler {
		fn ready(&self, ctx: serenity::Context, ready: serenity::Ready) {
			println!("Connected to Discord as {}", ready.user.name);
			// UNWRAP: propagate poison
			*self.state.write().unwrap() = Some(discord_handler::State::load(&ctx, ready.user.id)
				.expect("Failed to initialize"));
			println!("Logged into EO");

		}
	
		fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
			// hehe no, we don't want endless message chains
			// (originally I wanted to just ignore own messages, but that's awkward to implement so
			// let's just ignore all bot messages)
			if msg.author.bot { return }

			lock!(self, state);

			let mut was_explicitly_invoked = false;
			let result = state.message(&ctx, &msg, &mut was_explicitly_invoked);
			if let Err(mut error) = result {
				// this looks complicated, but all it does is map serenity's confusing
				// "[Serenity] No correct json was received!" error to one of my more descriptive
				// error types
				if let discord_handler::Error::SerenityError(e) = &error {
					if let serenity::Error::Http(e) = e {
						if let serenity::HttpError::UnsuccessfulRequest(e) = &**e {
							if e.error.code == -1 {
								error = discord_handler::Error::AttemptedToSendInvalidMessage;
							}
						}
					}
				}

				println!("Error {}", error);

				let error_msg = error.to_string();
				if was_explicitly_invoked {
					// Print the error message into the chat
					if let Err(inner_e) = msg.channel_id.say(&ctx.http, &error_msg) {
						println!("Failed with '{:?}' while sending error message '{}'", inner_e, &error_msg);
					}
				}
			}
		}

		fn guild_member_update(&self,
			ctx: serenity::Context,
			old: Option<serenity::Member>,
			new: serenity::Member
		) {
			lock!(self, state);
			if let Err(e) = state.guild_member_update(ctx, old, new) {
				println!("Error in guild member update: {:?}", e);
			}
		}

		fn reaction_add(&self,
			ctx: serenity::Context,
			reaction: serenity::Reaction,
		) {
			lock!(self, state);
			if let Err(e) = state.reaction_add(ctx, reaction) {
				println!("Error in reaction add: {:?}", e);
			}
		}
	}

	let handler = Handler { state: std::sync::RwLock::new(None) };

	// Login to Discord and start bot
	let mut client = serenity::Client::new(auth::DISCORD_BOT_TOKEN, handler)
		.expect("Unable to create Discord client");
	client.start()?;

	Ok(())
}
