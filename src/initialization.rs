//! This is a very ugly file. I tried my best to contain this abomination as best as possible.

use crate::serenity;

fn assume_same_type<T>(_: T, _: T) {}

struct FuckThis<T>(T);
unsafe impl<T> std::marker::Send for FuckThis<T> {}
unsafe impl<T> std::marker::Sync for FuckThis<T> {}

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
					// important! or the login attempt can't finish
					drop(guard);
					// if the bot is not ready yet, wait a bit and check again
					std::thread::sleep(std::time::Duration::from_millis(100));
				}
			};
		};
	};
}

struct Handler {
	auth: crate::Auth,
	state: std::sync::RwLock<Option<crate::discord_handler::State>>,
}

impl serenity::EventHandler for Handler {
	fn ready(&self, ctx: serenity::Context, ready: serenity::Ready) {
		println!("Connected to Discord as {}", ready.user.name);
		// UNWRAP: propagate poison
		*self.state.write().unwrap() = Some(
			crate::discord_handler::State::load(&ctx, self.auth.clone(), ready.user.id)
				.expect("Failed to initialize"),
		);
		println!("Logged into EO");
	}

	fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
		// hehe no, we don't want endless message chains
		// (originally I wanted to just ignore own messages, but that's awkward to implement so
		// let's just ignore all bot messages)
		if msg.author.bot {
			return;
		}

		lock!(self, state);

		let mut was_explicitly_invoked = false;
		let result = state.message(&ctx, &msg, &mut was_explicitly_invoked);
		if let Err(mut error) = result {
			// this looks complicated, but all it does is map serenity's confusing
			// "[Serenity] No correct json was received!" error to one of my more descriptive
			// error types
			if let Some(serenity::Error::Http(e)) = error.downcast_ref() {
				if let serenity::HttpError::UnsuccessfulRequest(e) = &**e {
					if e.error.code == -1 {
						error = "Attempted to send an invalid Discord message. One or more fields were probably empty".into();
					}
				}
			}

			println!("Error {}", error);

			let error_msg = error.to_string();
			if was_explicitly_invoked {
				// Print the error message into the chat
				if let Err(inner_e) = msg.channel_id.say(&ctx.http, &error_msg) {
					println!(
						"Failed with '{:?}' while sending error message '{}'",
						inner_e, &error_msg
					);
				}
			}
		}
	}

	fn guild_member_update(
		&self,
		ctx: serenity::Context,
		old: Option<serenity::Member>,
		new: serenity::Member,
	) {
		lock!(self, state);
		if let Err(e) = state.guild_member_update(ctx, old, new) {
			println!("Error in guild member update: {:?}", e);
		}
	}

	fn reaction_add(&self, ctx: serenity::Context, reaction: serenity::Reaction) {
		lock!(self, state);
		if let Err(e) = state.reaction_add(ctx, reaction) {
			println!("Error in reaction add: {:?}", e);
		}
	}
}

pub fn start_bot() -> Result<(), Box<dyn std::error::Error>> {
	let auth = crate::Auth {
		discord_bot_token: std::env::var("DISCORD_BOT_TOKEN")
			.map_err(|_| "Invalid discord bot token")?,
		eo_username: std::env::var("EO_USERNAME").map_err(|_| "Invalid eo username")?,
		eo_password: std::env::var("EO_PASSWORD").map_err(|_| "Invalid eo password")?,
		eo_client_data: std::env::var("EO_CLIENT_DATA").map_err(|_| "Invalid eo client data")?,
	};

	let handler = Handler {
		auth,
		state: std::sync::RwLock::new(None),
	};

	// Login to Discord and start bot
	let mut client = serenity::Client::new(handler.auth.discord_bot_token.clone(), handler)
		.expect("Unable to create Discord client");
	client.threadpool.set_num_threads(10);

	let thread_pool_ptr = unsafe { &*(&client.threadpool as *const _) }; // screw the rules
	assume_same_type(thread_pool_ptr, &client.threadpool);
	let thread_pool_ptr = FuckThis(thread_pool_ptr);

	std::thread::Builder::new()
		.name("stupid checker thread".to_owned())
		.spawn(move || {
			let thread_pool = thread_pool_ptr.0;

			let mut maxed_out_in_a_row = 0;
			loop {
				let (active, max) = (thread_pool.active_count(), thread_pool.max_count());
				// println!("Serenity thread pool: {}/{} threads active", active, max);
				if active == max {
					maxed_out_in_a_row += 1;
					if maxed_out_in_a_row >= 5 {
						// Thread pool was maxed out for three minutes straight. This can't be right
						// Let's spawn a new process to take over, but keep this instance running to
						// allow debugging
						println!("THIS INSTANCE IS STUCK STUCK STUCK!!!!");

						let current_exe =
							std::env::current_exe().expect("Can't get current exe path :(");
						std::process::Command::new(current_exe)
							.spawn()
							.expect("Failed to start bot clone");

						println!(
							"Started bot process to take over, stalling current instance's watchdog thread..."
						);
						loop {
							std::thread::park();
						}
					}
				} else {
					maxed_out_in_a_row = 0;
				}

				std::thread::sleep(std::time::Duration::from_secs(60));
			}
		})
		.unwrap();

	client.start()?;

	Ok(())
}
