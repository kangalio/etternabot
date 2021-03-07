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

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(
		"Attempted to send an invalid Discord message. One or more fields were probably empty"
	)]
	AttemptedToSendInvalidMessage,
	#[error(
		"User {discord_username} not found on EO. Please manually specify your EtternaOnline \
		username with `+userset`"
	)]
	CouldNotDeriveEoUsername { discord_username: String },
	#[error("EtternaOnline error: {0}")]
	EoApiError(#[from] etternaonline_api::Error),
	#[error("Can't complete this request because EO login failed ({0})")]
	FailedEoLogin(etternaonline_api::Error),
	#[error(transparent)]
	SerenityError(#[from] serenity::Error),
	#[error(transparent)]
	PatternError(#[from] discord_handler::PatternError),
	#[error("{0}")]
	ReplayGraphError(String),
	#[error("{0}")]
	SkillGraphError(String),
	#[error("Failed analyzing the score evaluation screenshot: {0:?}")]
	ScoreOcr(#[from] discord_handler::OcrError),
	#[error("A score was requested from EO but none was sent")]
	NoScoreEvenThoughOneWasRequested,
	#[error("User not found in registry (`+userset` must have been called at least once)")]
	UserNotInRegistry,
}

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
