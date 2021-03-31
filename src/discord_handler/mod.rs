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

// Returns None if msg was sent in DMs
fn get_guild_member(
	ctx: &serenity::Context,
	msg: &serenity::Message,
) -> Result<Option<serenity::Member>, serenity::Error> {
	Ok(match msg.guild_id {
		Some(guild_id) => Some(match msg.member(&ctx.cache) {
			Some(cached_member) => cached_member,
			None => ctx.http.get_member(guild_id.0, msg.author.id.0)?,
		}),
		None => None,
	})
}

// My Fucking GODDDDDDD WHY DOES SERENITY NOT PROVIDE THIS BASIC STUFF
fn get_guild_permissions(
	ctx: &serenity::Context,
	msg: &serenity::Message,
) -> Result<Option<serenity::Permissions>, serenity::Error> {
	fn aggregate_role_permissions(
		guild_member: &serenity::Member,
		guild_owner_id: serenity::UserId,
		guild_roles: &std::collections::HashMap<serenity::RoleId, serenity::Role>,
	) -> serenity::Permissions {
		if guild_owner_id == guild_member.user_id() {
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

	if let (Some(guild_member), Some(guild_id)) = (get_guild_member(ctx, msg)?, msg.guild_id) {
		// `guild_member.permissions(&ctx.cache)` / `guild.member_permissions(msg.author.id)` can't
		// be trusted - they return LITERALLY WRONG RESULTS AILUWRHDLIAUEHFISAUEHGLSIREUFHGLSIURHS
		// See this thread on the serenity dev server: https://discord.com/channels/381880193251409931/381912587505500160/787965510124830790
		let permissions = if let Some(guild) = guild_id.to_guild_cached(&ctx.cache) {
			// try get guild data from cache and calculate permissions ourselves
			let guild = guild.read();
			aggregate_role_permissions(&guild_member, guild.owner_id, &guild.roles)
		} else {
			// request guild data from http and calculate permissions ourselves
			let guild = &guild_id.to_partial_guild(&ctx.http)?;
			aggregate_role_permissions(&guild_member, guild.owner_id, &guild.roles)
		};

		Ok(Some(permissions))
	} else {
		Ok(None)
	}
}

/// The contained Option must be Some!!!
struct IdkWhatImDoing<'a> {
	guard: crate::AntiDeadlockMutexGuard<'a, Option<eo::v2::Session>>,
}
impl std::ops::Deref for IdkWhatImDoing<'_> {
	type Target = eo::v2::Session;

	fn deref(&self) -> &Self::Target {
		// UNWRAP: this will work because it's an invariant of this type
		self.guard.as_ref().unwrap()
	}
}

struct AutoSaveGuard<'a> {
	guard: crate::AntiDeadlockMutexGuard<'a, Data>,
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
fn user_has_manage_messages_permission(ctx: Context<'_>) -> Result<bool, Error> {
	Ok(get_guild_permissions(ctx.discord, ctx.msg)?.map_or(true, |p| p.manage_messages()))
}

/// If the message is in etternaonline server, and not in an allowed channel, and not sent
/// with elevated privileges, return false
fn user_is_allowed_bot_interaction(ctx: Context<'_>) -> Result<bool, Error> {
	Ok(
		if let Some(guild_member) = &get_guild_member(ctx.discord, ctx.msg)? {
			user_has_manage_messages_permission(ctx)?
				|| ctx
					.data
					.config
					.allowed_channels
					.contains(&ctx.msg.channel_id)
				|| guild_member.guild_id != ctx.data.config.etterna_online_guild_id
		} else {
			true
		},
	)
}

pub fn init_framework() -> poise::FrameworkOptions<State, Error> {
	poise::FrameworkOptions {
		command_check: user_is_allowed_bot_interaction,
		listener: |ctx, event, framework, state| match event {
			poise::Event::Message { new_message } => {
				let ctx = poise::Context {
					data: state,
					discord: ctx,
					msg: new_message,
					framework,
				};
				listeners::listen_message(
					ctx,
					user_has_manage_messages_permission(ctx)?,
					user_is_allowed_bot_interaction(ctx)?,
				)
			}
			poise::Event::GuildMemberUpdate {
				old_if_available,
				new,
			} => listeners::guild_member_update(state, ctx, old_if_available.as_ref(), &new),
			_ => Ok(()),
		},
		on_error: |e, ctx| match ctx {
			poise::ErrorContext::Command(ctx) => {
				let user_error_msg = if let Some(poise::ArgumentParseError(e)) = e.downcast_ref() {
					// If we caught an argument parse error, give a helpful error message with the
					// command explanation if available
					if let Some(explanation) = &ctx.command.options.explanation {
						format!("{}\n{}", e, explanation)
					} else {
						format!("You entered the command wrong, please check the help menu\n`{}`", e)
					}
				} else {
					e.to_string()
				};
				if let Err(e) = ctx.ctx.msg.channel_id.say(ctx.ctx.discord, user_error_msg) {
					println!("Error while posting argument parse error: {}", e);
				}
			}
			_ => println!("Something... happened?"),
		},
		broadcast_typing: true,
		edit_tracker: Some(poise::EditTracker::for_timespan(std::time::Duration::from_secs(3600))),
		commands: vec![
			poise::Command {
				name: "help",
				action: commands::help,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "profile",
				action: commands::profile,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "advprof",
				action: commands::profile,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "lastsession",
				action: commands::latest_scores,
				options: poise::CommandOptions {
					aliases: &["ls"],
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "pattern",
				action: |ctx, args| commands::pattern(ctx.data, ctx.discord, ctx.msg, args),
				options: poise::CommandOptions {
					check: Some(|_| Ok(true)), // allow pattern command everywhere
					..Default::default()
				},
			},
			poise::Command {
				name: "ping",
				action: commands::ping,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "servers",
				action: commands::servers,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "uptime",
				action: commands::uptime,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "randomscore",
				action: commands::random_score,
				options: poise::CommandOptions {
					..Default::default()
				},
			},
			poise::Command {
				name: "lookup",
				action: commands::lookup,
				options: poise::CommandOptions {
					explanation: Some("Call this command with `+lookup DISCORDUSERNAME`".into()),
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "quote",
				action: commands::quote,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "scrollset",
				action: commands::scrollset,
				options: poise::CommandOptions {
					track_edits: Some(true),
					explanation: Some("Call this command with `+scrollset [down/up]`".into()),
					..Default::default()
				},
			},
			poise::Command {
				name: "userset",
				action: commands::userset,
				options: poise::CommandOptions {
					explanation: Some("Call this command with `+userset YOUR_EO_USERNAME`".into()),
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "rivalset",
				action: commands::rivalset,
				options: poise::CommandOptions {
					explanation: Some("Call this command with `+rivalset YOUR_EO_USERNAME`".into()),
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "rs",
				action: commands::rs,
				options: poise::CommandOptions {
					explanation: Some("Call this command with `+rs [username] [judge]`".into()),
					..Default::default()
				},
			},
			poise::Command {
				name: "rival",
				action: commands::rival,
				options: poise::CommandOptions {
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "compare",
				action: commands::compare,
				options: poise::CommandOptions {
					explanation: Some("Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`. Add `expanded` at the end to see a graphic".into()),
					track_edits: Some(true),
					..Default::default()
				},
			},
			poise::Command {
				name: "skillgraph",
				action: |ctx, args| commands::skillgraph(ctx.data, ctx.discord, ctx.msg, args),
				options: poise::CommandOptions {
					..Default::default()
				},
			},
			poise::Command {
				name: "rivalgraph",
				action: |ctx, args| commands::rivalgraph(ctx.data, ctx.discord, ctx.msg, args),
				options: poise::CommandOptions {
					..Default::default()
				},
			},
			poise::Command {
				name: "accuracygraph",
				action: |ctx, args| commands::accuracygraph(ctx.data, ctx.discord, ctx.msg, args),
				options: poise::CommandOptions {
					aliases: &["accgraph"],
					..Default::default()
				},
			},
		],
		..Default::default()
	}

	// TODO: add topNN command
}

pub struct State {
	auth: crate::Auth,
	bot_start_time: std::time::Instant,
	config: Config,
	_data: crate::AntiDeadlockMutex<Data>,
	v2_session: crate::AntiDeadlockMutex<Option<eo::v2::Session>>, // stores the session, or None if login failed
	web_session: eo::web::Session,
	noteskin_provider: commands::NoteskinProvider,
	_bot_user_id: serenity::UserId,
}

impl State {
	pub fn load(
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
			.to_channel(ctx)?
			.guild()
			.is_none()
		{
			panic!("Configured promotion gratulations channel is not a valid guild channel!");
		}

		Ok(Self {
			bot_start_time: std::time::Instant::now(),
			v2_session: crate::AntiDeadlockMutex::new(match Self::attempt_v2_login(&auth) {
				Ok(v2) => Some(v2),
				Err(e) => {
					println!("Failed to login to EO on bot startup: {}. Continuing with no v2 session active", e);
					None
				}
			}),
			auth,
			web_session,
			config,
			_data: crate::AntiDeadlockMutex::new(Data::load()),
			_bot_user_id: bot_user_id,
			noteskin_provider: commands::NoteskinProvider::load()?,
		})
	}

	fn attempt_v2_login(auth: &crate::Auth) -> Result<eo::v2::Session, eo::Error> {
		eo::v2::Session::new_from_login(
			auth.eo_username.to_owned(),
			auth.eo_password.to_owned(),
			auth.eo_client_data.to_owned(),
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(30000)),
		)
	}

	// Automatically saves when the returned guard goes out of scope
	fn lock_data(&self) -> AutoSaveGuard<'_> {
		AutoSaveGuard {
			guard: self._data.lock(),
		}
	}

	/// attempt to retrieve the v2 session object. If there is none because login had failed,
	/// retry login just to make sure that EO is _really_ done
	/// the returned value contains a mutex guard. so if thread 1 calls v2() while thread 2 still
	/// holds the result from its call to v2(), thread 1 will block.
	fn v2(&self) -> Result<IdkWhatImDoing<'_>, Error> {
		let mut v2_session = self.v2_session.lock();

		if v2_session.is_some() {
			Ok(IdkWhatImDoing { guard: v2_session })
		} else {
			match Self::attempt_v2_login(&self.auth) {
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

	fn get_eo_username(
		&self,
		_ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<String, Error> {
		if let Some(user_entry) = self
			.lock_data()
			.user_registry
			.iter()
			.find(|user| user.discord_id == msg.author.id.0)
		{
			return Ok(user_entry.eo_username.to_owned());
		}

		match self.web_session.user_details(&msg.author.name) {
			Ok(user_details) => {
				// Seems like the user's EO name is the same as their Discord name :)
				// TODO: could replace the user_details call with scores request to get
				// last_known_num_scores as well here
				self.lock_data()
					.user_registry
					.push(config::UserRegistryEntry {
						discord_id: msg.author.id.0,
						discord_username: msg.author.name.to_owned(),
						eo_id: user_details.user_id,
						eo_username: msg.author.name.to_owned(),
						last_known_num_scores: None,
						last_rating: None,
					});

				Ok(msg.author.name.to_owned())
			}
			Err(eo::Error::UserNotFound) => Err(format!(
				"User {} not found on EO. Please manually specify your EtternaOnline username with `+userset`",
				msg.author.name.to_owned()
			)
			.into()),
			Err(other) => Err(other.into()),
		}
	}

	fn get_eo_user_id(&self, eo_username: &str) -> Result<u32, Error> {
		match self
			.lock_data()
			.user_registry
			.iter_mut()
			.find(|user| user.eo_username == eo_username)
		{
			Some(user) => Ok(user.eo_id),
			None => Ok(self.web_session.user_details(eo_username)?.user_id), // TODO: integrate into registry?
		}
	}
}
