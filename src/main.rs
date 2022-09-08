//! Root file, contains constants and utility functions used throughout the library and of course
//! the main function

#![allow(
	clippy::len_zero, // easier to read
	clippy::tabs_in_doc_comments, // we use tabs like it or not
	clippy::collapsible_if, // easier to read
	clippy::eval_order_dependence, // false positives
	clippy::needless_borrow, // no reason to fix and would litter commit history
)]
#![warn(rust_2018_idioms)]

mod config;
use config::Data;

mod commands;

mod score_card;
pub use score_card::*;

mod listeners;

mod framework;

mod state;
pub use state::State;

mod cached;
use cached::Cached;

// Custom serenity prelude module
use poise::serenity_prelude as serenity;

const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);
const MISSING_REGISTRY_ENTRY_ERROR_MESSAGE: &str =
	"User not found in registry (`+userset` must have been called at least once)";

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, State, Error>;
type PrefixContext<'a> = poise::PrefixContext<'a, State, Error>;

#[derive(Clone)]
pub struct Auth {
	eo_username: String,
	eo_password: String,
	eo_v1_api_key: String,
	eo_v2_client_data: String,
}

/// Returns a question mark emoji on invalid country code
fn country_code_to_flag_emoji(country_code: &str) -> String {
	fn inner(country_code: &str) -> Option<String> {
		if country_code.chars().any(|c| !c.is_alphabetic()) {
			return None;
		}

		let regional_indicator_value_offset = '🇦' as u32 - 'a' as u32;
		country_code
			.chars()
			.map(|c| {
				std::char::from_u32(c.to_ascii_lowercase() as u32 + regional_indicator_value_offset)
			})
			.collect()
	}
	inner(country_code).unwrap_or_else(|| "❓".into())
}

fn extract_judge_from_string(string: &str) -> Option<&'static etterna::Judge> {
	static JUDGE_REGEX: once_cell::sync::Lazy<regex::Regex> =
		once_cell::sync::Lazy::new(|| regex::Regex::new(r"[jJ](\d)").unwrap());

	JUDGE_REGEX
		.captures_iter(string)
		.filter_map(|groups| {
			// UNWRAP: the regex definition contains a group
			let judge_num_string = groups.get(1).unwrap().as_str();

			let judge_num: u32 = judge_num_string.parse().ok()?;

			match judge_num {
				1 => Some(etterna::J1),
				2 => Some(etterna::J2),
				3 => Some(etterna::J3),
				4 => Some(etterna::J4),
				5 => Some(etterna::J5),
				6 => Some(etterna::J6),
				7 => Some(etterna::J7),
				8 => Some(etterna::J8),
				9 => Some(etterna::J9),
				_ => None,
			}
		})
		.next()
}

/// Transforms an error by checking, if it's a User Not Found error. If yes,
fn no_such_user_or_skillset(error: etternaonline_api::Error) -> Error {
	log::warn!("Got an error {}", error);
	match error {
		etternaonline_api::Error::UserNotFound {
			name: Some(username),
		} => format!("No such user or skillset \"{}\"", username).into(),
		etternaonline_api::Error::UserNotFound { name: None } => "No such user or skillset".into(),
		other => other.into(),
	}
}

async fn autocomplete_username(ctx: Context<'_>, partial: &str) -> Vec<String> {
	let usernames = ctx.data().eo_usernames.fetch(ctx).await;

	let partial = partial.to_lowercase();
	usernames
		.iter()
		.filter(move |username| username.starts_with(&partial))
		.map(|s| s.clone())
		.collect()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init();

	fn env_var<T: std::str::FromStr>(name: &str) -> Result<T, Error>
	where
		T::Err: std::fmt::Display,
	{
		Ok(std::env::var(name)
			.map_err(|_| format!("Invalid/missing {}", name))?
			.parse()
			.map_err(|e| format!("Invalid {}: {}", name, e))?)
	}

	let auth = crate::Auth {
		eo_username: env_var("EO_USERNAME")?,
		eo_password: env_var("EO_PASSWORD")?,
		eo_v1_api_key: env_var("EO_API_KEY")?,
		eo_v2_client_data: env_var("EO_CLIENT_DATA")?,
	};
	let discord_bot_token: String = env_var("DISCORD_BOT_TOKEN")?;

	framework::run_framework(auth, &discord_bot_token).await?;

	Ok(())
}
