mod config;

mod commands;
pub use commands::PatternError;

mod score_card;
pub use score_card::*;

mod listeners;

use crate::{serenity, Error};
use config::{Config, Data};
use etternaonline_api as eo;

fn extract_judge_from_string(string: &str) -> Option<&etterna::Judge> {
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
	fn lock_data(&self) -> AutoSaveGuard {
		AutoSaveGuard {
			guard: self._data.lock(),
		}
	}

	/// attempt to retrieve the v2 session object. If there is none because login had failed,
	/// retry login just to make sure that EO is _really_ done
	/// the returned value contains a mutex guard. so if thread 1 calls v2() while thread 2 still
	/// holds the result from its call to v2(), thread 1 will block.
	fn v2(&self) -> Result<IdkWhatImDoing, Error> {
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

	fn command(
		&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		args: &str,
	) -> Result<(), Error> {
		println!(
			"Executing command '{}' from {} at {:?} with args '{}'",
			cmd,
			&msg.author.name,
			msg.timestamp.date(),
			args
		);

		if let Some(limit) = cmd.strip_prefix("top") {
			if let Ok(limit) = limit.parse() {
				commands::top_scores(self, ctx, msg, args, limit)?;
			} else {
				msg.channel_id.say(&ctx.http, commands::CMD_TOP_HELP)?;
			}
			return Ok(());
		}

		match cmd {
			"help" => commands::help(self, ctx, msg, args)?,
			"profile" => commands::profile(self, ctx, msg, args)?,
			"advprof" => {
				msg.channel_id.say(&ctx.http, "Note: +profile now does the same thing as +advprof; there's no reason to use +advprof anymore")?;
				commands::profile(self, ctx, msg, args)?;
			}
			"lastsession" | "ls" => commands::latest_scores(self, ctx, msg, args)?,
			"pattern" => commands::pattern(self, ctx, msg, args)?,
			"ping" => commands::ping(self, ctx, msg, args)?,
			"servers" => commands::servers(self, ctx, msg, args)?,
			"uptime" => commands::uptime(self, ctx, msg, args)?,
			"random_score" => commands::random_score(self, ctx, msg, args)?,
			"lookup" => commands::lookup(self, ctx, msg, args)?,
			"quote" => commands::quote(self, ctx, msg, args)?,
			"scrollset" => commands::scrollset(self, ctx, msg, args)?,
			"userset" => commands::userset(self, ctx, msg, args)?,
			"rivalset" => commands::rivalset(self, ctx, msg, args)?,
			"rs" => commands::rs(self, ctx, msg, args)?,
			"latest_scores" => commands::latest_scores(self, ctx, msg, args)?,
			"rival" => commands::rival(self, ctx, msg, args)?,
			"compare" => commands::compare(self, ctx, msg, args)?,
			"skillgraph" => commands::skillgraph(self, ctx, msg, args)?,
			"rivalgraph" => commands::rivalgraph(self, ctx, msg, args)?,
			"accuracygraph" | "accgraph" => commands::accuracygraph(self, ctx, msg, args)?,
			_ => {}
		}
		Ok(())
	}

	pub fn message(
		&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		was_explicitly_invoked: &mut bool,
	) -> Result<(), Error> {
		// Let's not do this, because if a non existant command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		let guild_member = get_guild_member(ctx, msg)?;
		// true if sent in DMs
		let manages_messages =
			get_guild_permissions(ctx, msg)?.map_or(true, |p| p.manage_messages());

		// If the message is in etternaonline server, and not in an allowed channel, and not sent
		// with elevated privileges, then don't process the command
		let user_is_allowed_bot_interaction = {
			// if msg is in server (opposed to DMs)
			if let Some(guild_member) = &guild_member {
				if guild_member.guild_id == self.config.etterna_online_guild_id
					&& !self.config.allowed_channels.contains(&msg.channel_id)
					&& !manages_messages
				{
					false
				} else {
					true
				}
			} else {
				true
			}
		};

		listeners::listen_message(
			self,
			ctx,
			msg,
			manages_messages,
			user_is_allowed_bot_interaction,
		)?;

		if msg.content.starts_with('+') {
			*was_explicitly_invoked = true;

			// UNWRAP: we just checked it has a string at the beginning that we can chop away
			let text = &msg.content.get(1..).unwrap();

			// Split message into command part and parameter part
			let mut a = text.splitn(2, ' ');
			// UNWRAP: msg.content can't be empty, hence the token iterator has at least one elem
			let command_name = a.next().unwrap().trim();
			let parameters = a.next().unwrap_or("").trim();

			// only the pattern command is allowed everywhere
			// this implementation is bad because this function shouldn't know about the specific
			// commands that exist...
			if user_is_allowed_bot_interaction || command_name == "pattern" {
				self.command(&ctx, &msg, command_name, parameters)?;
			}
		}

		Ok(())
	}

	pub fn guild_member_update(
		&self,
		ctx: serenity::Context,
		old: Option<serenity::Member>,
		new: serenity::Member,
	) -> Result<(), Error> {
		listeners::guild_member_update(self, ctx, old, new)
	}
}
