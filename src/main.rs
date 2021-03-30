#![allow(
	clippy::len_zero,
	clippy::tabs_in_doc_comments,
	clippy::collapsible_if,
	clippy::needless_bool
)]
#![warn(rust_2018_idioms)]

mod anti_deadlock_mutex;
mod discord_handler;
// mod initialization;

pub use anti_deadlock_mutex::*;

// Custom serenity prelude module
mod serenity {
	pub use serenity::{
		builder::*,
		model::{event::*, prelude::*},
		prelude::*,
		utils::*,
		Error,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub const MISSING_REGISTRY_ENTRY_ERROR_MESSAGE: &str =
	"User not found in registry (`+userset` must have been called at least once)";

#[derive(Clone)]
pub struct Auth {
	eo_username: String,
	eo_password: String,
	eo_client_data: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let auth = crate::Auth {
		eo_username: std::env::var("EO_USERNAME").map_err(|_| "Invalid/missing eo username")?,
		eo_password: std::env::var("EO_PASSWORD").map_err(|_| "Invalid/missing eo password")?,
		eo_client_data: std::env::var("EO_CLIENT_DATA")
			.map_err(|_| "Invalid/missing eo client data")?,
	};
	let discord_bot_token =
		std::env::var("DISCORD_BOT_TOKEN").map_err(|_| "Invalid discord bot token")?;

	let framework = poise::Framework::new(
		"+",
		|ctx, ready| {
			discord_handler::State::load(ctx, auth, ready.user.id).expect("Failed to initialize")
		},
		discord_handler::init_framework(),
	);
	framework.start(&discord_bot_token)?;

	Ok(())
}
