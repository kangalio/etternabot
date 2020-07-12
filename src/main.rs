mod discord_handler;
mod auth;
mod api;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		prelude::*,
		model::{gateway::Ready, channel::Message}
	};
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Login to EO
	let session = api::Session::new_from_login(
		auth::EO_USERNAME.to_owned(),
		auth::EO_PASSWORD.to_owned(),
		auth::EO_CLIENT_DATA.to_owned(),
	)?;
	let handler = discord_handler::Handler::from_session(session);

	// Login to Discord and start bot
	let mut client = serenity::Client::new(auth::DISCORD_BOT_TOKEN, handler)
		.expect("Unable to create Discord client");
	client.start()?;

	Ok(())
}
