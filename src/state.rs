//! Stores the State type which is passed to every command invocation and contains all quasi-global
//! data that commands need, like the EtternaOnline client sessions or configuration

use super::{commands, config};
use crate::{serenity, Error};

const EO_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(1000);
const EO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(30000);

/// The contained Option must be Some!!!
pub struct IdkWhatImDoing<'a> {
	guard: tokio::sync::MutexGuard<'a, Option<etternaonline_api::v2::Session>>,
}
impl std::ops::Deref for IdkWhatImDoing<'_> {
	type Target = etternaonline_api::v2::Session;

	fn deref(&self) -> &Self::Target {
		// UNWRAP: this will work because it's an invariant of this type
		self.guard.as_ref().unwrap()
	}
}

pub struct AutoSaveGuard<'a> {
	guard: std::sync::MutexGuard<'a, crate::Data>,
}
impl std::ops::Deref for AutoSaveGuard<'_> {
	type Target = config::Data;

	fn deref(&self) -> &Self::Target {
		&*self.guard
	}
}
impl std::ops::DerefMut for AutoSaveGuard<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut *self.guard
	}
}
impl Drop for AutoSaveGuard<'_> {
	fn drop(&mut self) {
		self.guard.save();
	}
}

pub struct State {
	auth: crate::Auth,
	pub bot_start_time: std::time::Instant,
	pub config: config::Config,
	data: std::sync::Mutex<config::Data>,
	// stores the session, or None if login failed
	pub v1: etternaonline_api::v1::Session,
	v2_session: tokio::sync::Mutex<Option<etternaonline_api::v2::Session>>,
	pub web: etternaonline_api::web::Session,
	pub noteskin_provider: commands::NoteskinProvider,
}

impl State {
	pub async fn load(ctx: &serenity::Context, auth: crate::Auth) -> Result<Self, Error> {
		let web_session = etternaonline_api::web::Session::new(
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(300_000)), // EO takes a while for user scores
		);

		let config = config::Config::load();
		if config
			.promotion_gratulations_channel
			.to_channel(ctx)
			.await?
			.guild()
			.is_none()
		{
			panic!("Configured promotion gratulations channel is not a valid guild channel!");
		}

		Ok(Self {
			bot_start_time: std::time::Instant::now(),
			v1: etternaonline_api::v1::Session::new(
				auth.eo_v1_api_key.clone(),
				EO_COOLDOWN,
				Some(EO_TIMEOUT),
			),
			v2_session: tokio::sync::Mutex::new(match Self::attempt_v2_login(&auth).await {
				Ok(v2) => Some(v2),
				Err(e) => {
					log::warn!("Failed to login to EO on bot startup: {}. Continuing with no v2 session active", e);
					None
				}
			}),
			auth,
			web: web_session,
			config,
			data: std::sync::Mutex::new(config::Data::load()),
			noteskin_provider: commands::NoteskinProvider::load()?,
		})
	}

	async fn attempt_v2_login(
		auth: &crate::Auth,
	) -> Result<etternaonline_api::v2::Session, etternaonline_api::Error> {
		etternaonline_api::v2::Session::new_from_login(
			auth.eo_username.to_owned(),
			auth.eo_password.to_owned(),
			auth.eo_v2_client_data.to_owned(),
			EO_COOLDOWN,
			Some(EO_TIMEOUT),
		)
		.await
	}

	// Automatically saves when the returned guard goes out of scope
	pub fn lock_data(&self) -> AutoSaveGuard<'_> {
		AutoSaveGuard {
			guard: self.data.lock().unwrap(),
		}
	}

	/// attempt to retrieve the v2 session object. If there is none because login had failed,
	/// retry login just to make sure that EO is _really_ down
	/// the returned value contains a mutex guard. so if thread 1 calls v2() while thread 2 still
	/// holds the result from its call to v2(), thread 1 will block.
	pub async fn v2(&self) -> Result<IdkWhatImDoing<'_>, Error> {
		let mut v2_session = self.v2_session.lock().await;

		if v2_session.is_some() {
			Ok(IdkWhatImDoing { guard: v2_session })
		} else {
			match Self::attempt_v2_login(&self.auth).await {
				Ok(v2) => {
					*v2_session = Some(v2);
					Ok(IdkWhatImDoing { guard: v2_session })
				}
				Err(e) => {
					*v2_session = None;

					let e = format!(
						"Can't complete this request because EO login failed ({})",
						e
					);
					Err(e.into())
				}
			}
		}
	}

	pub async fn get_eo_username(&self, discord_user: &serenity::User) -> Result<String, Error> {
		if let Some(user_entry) = self
			.lock_data()
			.user_registry
			.iter()
			.find(|user| user.discord_id == discord_user.id)
		{
			return Ok(user_entry.eo_username.to_owned());
		}

		match self.web.user_details(&discord_user.name).await {
			Ok(user_details) => {
				// Seems like the user's EO name is the same as their Discord name :)
				// TODO: could replace the user_details call with scores request to get
				// last_known_num_scores as well here
				self.lock_data()
					.user_registry
					.push(config::UserRegistryEntry {
						discord_id: discord_user.id,
						discord_username: discord_user.name.to_owned(),
						eo_id: user_details.user_id,
						eo_username: discord_user.name.to_owned(),
						last_known_num_scores: None,
						last_rating: None,
					});

				Ok(discord_user.name.to_owned())
			}
			Err(etternaonline_api::Error::UserNotFound { name: _ }) => Err(format!(
				"User {} not found on EO. Please manually specify your EtternaOnline username with `+userset`",
				discord_user.name.to_owned()
			)
			.into()),
			Err(other) => Err(other.into()),
		}
	}

	pub async fn get_eo_user_id(&self, eo_username: &str) -> Result<u32, Error> {
		if let Some(user) = self
			.lock_data()
			.user_registry
			.iter_mut()
			.find(|user| user.eo_username == eo_username)
		{
			return Ok(user.eo_id);
		}

		Ok(self.web.user_details(eo_username).await?.user_id) // TODO: integrate into registry?
	}
}
