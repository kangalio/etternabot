#[allow(clippy::len_zero, clippy::tabs_in_doc_comments)]

mod discord_handler;
mod auth;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		prelude::*,
		model::{gateway::Ready, channel::Message},
		framework::standard::{Args, Delimiter},
		utils::Colour as Color,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let handler = discord_handler::Handler::load()?;

	// Login to Discord and start bot
	let mut client = serenity::Client::new(auth::DISCORD_BOT_TOKEN, handler)
		.expect("Unable to create Discord client");
	client.start()?;

	Ok(())
}
