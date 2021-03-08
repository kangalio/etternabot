#![allow(
	clippy::len_zero,
	clippy::tabs_in_doc_comments,
	clippy::collapsible_if,
	clippy::needless_bool
)]

mod anti_deadlock_mutex;
mod discord_handler;
mod initialization;

pub use anti_deadlock_mutex::*;

// This is my custom serenity prelude module
mod serenity {
	pub use serenity::{
		http::error::{DiscordJsonError, Error as HttpError, ErrorResponse},
		model::prelude::*,
		prelude::*,
		utils::Colour as Color,
		Error,
	};
}

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub const MISSING_REGISTRY_ENTRY_ERROR_MESSAGE: &str =
	"User not found in registry (`+userset` must have been called at least once)";

#[derive(Clone)]
pub struct Auth {
	discord_bot_token: String,
	eo_username: String,
	eo_password: String,
	eo_client_data: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	initialization::start_bot()
}
