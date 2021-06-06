#![allow(
	clippy::len_zero,
	clippy::tabs_in_doc_comments,
	clippy::collapsible_if,
	clippy::needless_bool
)]
#![warn(rust_2018_idioms)]

mod anti_deadlock_mutex;
mod discord_handler;

pub use anti_deadlock_mutex::*;

// Custom serenity prelude module
use poise::serenity_prelude as serenity;

pub const ETTERNA_COLOR: serenity::Color = serenity::Color::from_rgb(78, 0, 146);

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub const MISSING_REGISTRY_ENTRY_ERROR_MESSAGE: &str =
	"User not found in registry (`+userset` must have been called at least once)";

#[derive(Clone)]
pub struct Auth {
	eo_username: String,
	eo_password: String,
	eo_v1_api_key: String,
	eo_v2_client_data: String,
	imgbb_api_key: String,
}

fn env_var<T: std::str::FromStr>(name: &str) -> Result<T, Error>
where
	T::Err: std::fmt::Display,
{
	Ok(std::env::var(name)
		.map_err(|_| format!("Invalid/missing {}", name))?
		.parse()
		.map_err(|e| format!("Invalid {}: {}", name, e))?)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	let auth = crate::Auth {
		eo_username: env_var("EO_USERNAME")?,
		eo_password: env_var("EO_PASSWORD")?,
		eo_v1_api_key: env_var("EO_API_KEY")?,
		eo_v2_client_data: env_var("EO_CLIENT_DATA")?,
		imgbb_api_key: env_var("IMGBB_API_KEY")?,
	};
	let discord_bot_token: String = env_var("DISCORD_BOT_TOKEN")?;
	let application_id = env_var("APPLICATION_ID")?;

	let framework = poise::Framework::new(
		"+",
		serenity::ApplicationId(application_id),
		|ctx, ready, _| Box::pin(discord_handler::State::load(ctx, auth, ready.user.id)),
		discord_handler::init_framework(),
	);
	framework
		.start(serenity::Client::builder(discord_bot_token).intents(
			serenity::GatewayIntents::non_privileged()
				| serenity::GatewayIntents::GUILD_MEMBERS
				| serenity::GatewayIntents::GUILD_PRESENCES,
		))
		.await?;

	Ok(())
}
