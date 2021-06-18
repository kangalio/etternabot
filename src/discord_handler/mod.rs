mod config;

mod commands;
pub use commands::PatternError;

mod score_card;
pub use score_card::*;

mod listeners;

use crate::{serenity, Error};
use config::{Config, Data};
use etternaonline_api as eo;

type Context<'a> = poise::Context<'a, State, Error>;
type PrefixContext<'a> = poise::PrefixContext<'a, State, Error>;
// type SlashContext<'a> = poise::SlashContext<'a, State, Error>;

const EO_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(1000);
const EO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(30000);

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
	println!("Got an error {}", error);
	match error {
		etternaonline_api::Error::UserNotFound {
			name: Some(username),
		} => format!("No such user or skillset \"{}\"", username).into(),
		etternaonline_api::Error::UserNotFound { name: None } => "No such user or skillset".into(),
		other => other.into(),
	}
}

// Returns None if msg was sent in DMs
async fn get_guild_member(ctx: Context<'_>) -> Result<Option<serenity::Member>, serenity::Error> {
	match ctx.guild_id() {
		Some(guild_id) => guild_id
			.member(ctx.discord(), ctx.author().id)
			.await
			.map(Some),
		None => Ok(None),
	}
}

// My Fucking GODDDDDDD WHY DOES SERENITY NOT PROVIDE THIS BASIC STUFF
async fn get_guild_permissions(
	ctx: Context<'_>,
) -> Result<Option<serenity::Permissions>, serenity::Error> {
	fn aggregate_role_permissions(
		guild_member: &serenity::Member,
		guild_owner_id: serenity::UserId,
		guild_roles: &std::collections::HashMap<serenity::RoleId, serenity::Role>,
	) -> serenity::Permissions {
		if guild_owner_id == guild_member.user.id {
			// author is owner -> all permissions
			serenity::Permissions::all()
		} else {
			guild_member
				.roles
				.iter()
				.filter_map(|r| guild_roles.get(r))
				.fold(serenity::Permissions::empty(), |a, b| a | b.permissions)
		}
	}

	if let (Some(guild_member), Some(guild_id)) = (get_guild_member(ctx).await?, ctx.guild_id()) {
		// `guild_member.permissions(&ctx.cache)` / `guild.member_permissions(msg.author.id)` can't
		// be trusted - they return LITERALLY WRONG RESULTS AILUWRHDLIAUEHFISAUEHGLSIREUFHGLSIURHS
		// See this thread on the serenity dev server: https://discord.com/channels/381880193251409931/381912587505500160/787965510124830790
		let permissions = if let Some(guild) = guild_id.to_guild_cached(&ctx.discord()).await {
			// try get guild data from cache and calculate permissions ourselves
			aggregate_role_permissions(&guild_member, guild.owner_id, &guild.roles)
		} else {
			// request guild data from http and calculate permissions ourselves
			let guild = &guild_id.to_partial_guild(&ctx.discord()).await?;
			aggregate_role_permissions(&guild_member, guild.owner_id, &guild.roles)
		};

		Ok(Some(permissions))
	} else {
		Ok(None)
	}
}

/// The contained Option must be Some!!!
struct IdkWhatImDoing<'a> {
	guard: tokio::sync::MutexGuard<'a, Option<eo::v2::Session>>,
}
impl std::ops::Deref for IdkWhatImDoing<'_> {
	type Target = eo::v2::Session;

	fn deref(&self) -> &Self::Target {
		// UNWRAP: this will work because it's an invariant of this type
		self.guard.as_ref().unwrap()
	}
}

struct AutoSaveGuard<'a> {
	guard: std::sync::MutexGuard<'a, Data>,
}
impl std::ops::Deref for AutoSaveGuard<'_> {
	type Target = Data;

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

/// true if sent in DMs
async fn user_has_manage_messages_permission(ctx: Context<'_>) -> Result<bool, Error> {
	Ok(get_guild_permissions(ctx)
		.await?
		.map_or(true, |p| p.manage_messages()))
}

/// If the message is in etternaonline server, and not in an allowed channel, and not sent
/// with elevated privileges, return false
async fn user_is_allowed_bot_interaction(ctx: Context<'_>) -> Result<bool, Error> {
	Ok(if let Some(guild_member) = &get_guild_member(ctx).await? {
		user_has_manage_messages_permission(ctx).await?
			|| ctx
				.data()
				.config
				.allowed_channels
				.contains(&ctx.channel_id())
			|| guild_member.guild_id != ctx.data().config.etterna_online_guild_id
	} else {
		true
	})
}

async fn on_error(e: Error, ctx: poise::ErrorContext<'_, State, Error>) {
	println!("Encountered an error: {:?}", e);
	match ctx {
		poise::ErrorContext::Command(ctx) => {
			let user_error_msg = if let Some(poise::ArgumentParseError(e)) = e.downcast_ref() {
				// If we caught an argument parse error, give a helpful error message with the
				// command explanation if available

				let mut usage = "Please check the help menu for usage information".into();
				if let poise::CommandErrorContext::Prefix(ctx) = &ctx {
					if let Some(multiline_help) = &ctx.command.options.multiline_help {
						usage = multiline_help();
					}
				}
				format!("**{}**\n{}", e, usage)
			} else {
				e.to_string()
			};
			if let Err(e) = poise::say_reply(ctx.ctx(), user_error_msg).await {
				println!("Error while user command error: {}", e);
			}
		}
		poise::ErrorContext::Listener(event) => {
			println!("Error in listener while processing {:?}: {}", event, e)
		}
		poise::ErrorContext::Setup => println!("Setup failed: {}", e),
	}
}

async fn listener(
	ctx: &serenity::Context,
	event: &poise::Event<'_>,
	framework: &poise::Framework<State, Error>,
	state: &State,
) -> Result<(), Error> {
	match event {
		poise::Event::Message { new_message } => {
			let ctx = poise::PrefixContext {
				data: state,
				discord: ctx,
				msg: new_message,
				framework,
				command: None,
			};
			#[allow(clippy::eval_order_dependence)] // ???
			listeners::listen_message(
				ctx,
				user_has_manage_messages_permission(poise::Context::Prefix(ctx)).await?,
				user_is_allowed_bot_interaction(poise::Context::Prefix(ctx)).await?,
			)
			.await
		}
		poise::Event::GuildMemberUpdate {
			old_if_available,
			new,
		} => listeners::guild_member_update(state, ctx, old_if_available.as_ref(), &new).await,
		_ => Ok(()),
	}
}

async fn pre_command(ctx: poise::Context<'_, State, Error>) {
	let author = ctx.author();
	match ctx {
		poise::Context::Slash(ctx) => {
			let command_name = match &ctx.interaction.data {
				Some(data) => &data.name,
				None => "<not an interaction>",
			};
			println!(
				"{} invoked command {} on {:?}",
				&author.name,
				command_name,
				&ctx.interaction.id.created_at()
			);
		}
		poise::Context::Prefix(ctx) => {
			println!(
				"{} sent message {:?} on {:?}",
				&author.name, &ctx.msg.content, &ctx.msg.timestamp
			);
		}
	}
}

pub fn init_framework() -> poise::FrameworkOptions<State, Error> {
	let mut framework = poise::FrameworkOptions {
		listener: |ctx, event, framework, state| Box::pin(listener(ctx, event, framework, state)),
		on_error: |e, ctx| Box::pin(on_error(e, ctx)),
		prefix_options: poise::PrefixFrameworkOptions {
			command_check: |c| Box::pin(user_is_allowed_bot_interaction(poise::Context::Prefix(c))),
			broadcast_typing: poise::BroadcastTypingBehavior::WithDelay(
				std::time::Duration::from_secs_f32(1.0),
			),
			edit_tracker: Some(poise::EditTracker::for_timespan(
				std::time::Duration::from_secs(3600),
			)),
			..Default::default()
		},
		slash_options: poise::SlashFrameworkOptions {
			command_check: |c| Box::pin(user_is_allowed_bot_interaction(poise::Context::Slash(c))),
			defer_response: true,
			..Default::default()
		},
		pre_command: |ctx| Box::pin(pre_command(ctx)), // ..Default::default()
	};
	framework.command(commands::compare);
	framework.command(commands::help);
	framework.command(commands::profile);
	framework.command(commands::pattern);
	framework.command(commands::ping);
	framework.command(commands::servers);
	framework.command(commands::uptime);
	framework.command(commands::lastsession);
	framework.command(commands::randomscore);
	framework.command(commands::lookup);
	framework.command(commands::scrollset);
	framework.command(commands::userset);
	framework.command(commands::rivalset);
	framework.command(commands::rs);
	framework.command(commands::rival);
	framework.command(commands::skillgraph);
	framework.command(commands::rivalgraph);
	framework.command(commands::accuracygraph);
	framework.command(commands::quote);
	framework.command(commands::register);
	framework.command(commands::top);
	framework.command(commands::top10);
	framework.command(commands::aroundme);
	framework.command(commands::leaderboard);
	framework.command(commands::details);
	framework
}

pub struct State {
	auth: crate::Auth,
	bot_start_time: std::time::Instant,
	config: Config,
	_data: std::sync::Mutex<Data>,
	// stores the session, or None if login failed
	v1: eo::v1::Session,
	v2_session: tokio::sync::Mutex<Option<eo::v2::Session>>,
	web: eo::web::Session,
	noteskin_provider: commands::NoteskinProvider,
	_bot_user_id: serenity::UserId,
}

impl State {
	pub async fn load(
		ctx: &serenity::Context,
		auth: crate::Auth,
		bot_user_id: serenity::UserId,
	) -> Result<Self, Error> {
		let web_session = eo::web::Session::new(
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(300_000)), // EO takes a while for user scores
		);

		let config = Config::load();
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
					println!("Failed to login to EO on bot startup: {}. Continuing with no v2 session active", e);
					None
				}
			}),
			auth,
			web: web_session,
			config,
			_data: std::sync::Mutex::new(Data::load()),
			_bot_user_id: bot_user_id,
			noteskin_provider: commands::NoteskinProvider::load()?,
		})
	}

	async fn attempt_v2_login(auth: &crate::Auth) -> Result<eo::v2::Session, eo::Error> {
		eo::v2::Session::new_from_login(
			auth.eo_username.to_owned(),
			auth.eo_password.to_owned(),
			auth.eo_v2_client_data.to_owned(),
			EO_COOLDOWN,
			Some(EO_TIMEOUT),
		)
		.await
	}

	// Automatically saves when the returned guard goes out of scope
	fn lock_data(&self) -> AutoSaveGuard<'_> {
		AutoSaveGuard {
			guard: self._data.lock().unwrap(),
		}
	}

	/// attempt to retrieve the v2 session object. If there is none because login had failed,
	/// retry login just to make sure that EO is _really_ down
	/// the returned value contains a mutex guard. so if thread 1 calls v2() while thread 2 still
	/// holds the result from its call to v2(), thread 1 will block.
	async fn v2(&self) -> Result<IdkWhatImDoing<'_>, Error> {
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

	async fn get_eo_username(&self, discord_user: &serenity::User) -> Result<String, Error> {
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
			Err(eo::Error::UserNotFound { name: _ }) => Err(format!(
				"User {} not found on EO. Please manually specify your EtternaOnline username with `+userset`",
				discord_user.name.to_owned()
			)
			.into()),
			Err(other) => Err(other.into()),
		}
	}

	async fn get_eo_user_id(&self, eo_username: &str) -> Result<u32, Error> {
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
