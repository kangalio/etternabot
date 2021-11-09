//! This file sets up the framework, including configuration, registering all commands, registering
//! event handlers, global error handling, framework logging

use super::{commands, listeners};
use crate::{serenity, Context, Error, State};

// Returns None if msg was sent in DMs
async fn get_guild_member(ctx: Context<'_>) -> Result<Option<serenity::Member>, Error> {
	Ok(match ctx.guild_id() {
		Some(guild_id) => Some(guild_id.member(ctx.discord(), ctx.author()).await?),
		None => None,
	})
}

// My Fucking GODDDDDDD WHY DOES SERENITY NOT PROVIDE THIS BASIC STUFF
async fn get_guild_permissions(ctx: Context<'_>) -> Result<Option<serenity::Permissions>, Error> {
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
		let permissions = if let Some(guild) = guild_id.to_guild_cached(&ctx.discord()) {
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
	log::warn!("Encountered an error: {:?}", e);
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
				log::warn!("Error while user command error: {}", e);
			}
		}
		poise::ErrorContext::Listener(event) => {
			log::warn!("Error in listener while processing {:?}: {}", event, e)
		}
		poise::ErrorContext::Autocomplete(err_ctx) => {
			let ctx = err_ctx.ctx;
			log::warn!(
				"Error in autocomplete callback for command {:?}: {}",
				ctx.command.slash_or_context_menu_name(),
				e
			)
		}
		poise::ErrorContext::Setup => log::error!("Setup failed: {}", e),
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
		poise::Context::Application(ctx) => {
			log::info!(
				"{} invoked command {} on {:?}",
				&author.name,
				&ctx.interaction.data().name,
				&ctx.interaction.id().created_at()
			);
		}
		poise::Context::Prefix(ctx) => {
			let guild_name = match ctx.msg.guild(ctx.discord) {
				Some(guild) => guild.name,
				None => "<unknown>".into(),
			};
			log::info!(
				"{} sent message {:?} on {:?} in {}",
				&author.name,
				&ctx.msg.content,
				&ctx.msg.timestamp,
				guild_name,
			);
		}
	}
}

pub async fn run_framework(auth: crate::Auth, discord_bot_token: &str) -> Result<(), Error> {
	poise::Framework::build()
		.user_data_setup(|ctx, _ready, _| Box::pin(State::load(ctx, auth)))
		.options(poise::FrameworkOptions {
			listener: |ctx, event, framework, state| {
				Box::pin(listener(ctx, event, framework, state))
			},
			on_error: |e, ctx| Box::pin(on_error(e, ctx)),
			prefix_options: poise::PrefixFrameworkOptions {
				prefix: Some("+".into()),
				edit_tracker: Some(poise::EditTracker::for_timespan(
					std::time::Duration::from_secs(3600),
				)),
				..Default::default()
			},
			command_check: Some(|ctx| Box::pin(user_is_allowed_bot_interaction(ctx))),
			pre_command: |ctx| Box::pin(pre_command(ctx)),
			owners: std::iter::FromIterator::from_iter([serenity::UserId(472029906943868929)]),
			..Default::default()
		})
		.token(discord_bot_token)
		.client_settings(|client| {
			client.intents(
				serenity::GatewayIntents::non_privileged()
					| serenity::GatewayIntents::GUILD_MEMBERS
					| serenity::GatewayIntents::GUILD_PRESENCES,
			)
		})
		.command(commands::compare(), |f| f)
		.command(commands::help(), |f| f)
		.command(commands::profile(), |f| f)
		.command(commands::pattern(), |f| f)
		.command(commands::ping(), |f| f)
		.command(commands::servers(), |f| f)
		.command(commands::uptime(), |f| f)
		.command(commands::lastsession(), |f| f)
		.command(commands::randomscore(), |f| f)
		.command(commands::lookup(), |f| f)
		.command(commands::scrollset(), |f| f)
		.command(commands::userset(), |f| f)
		.command(commands::rivalset(), |f| f)
		.command(commands::rs(), |f| f)
		.command(commands::rival(), |f| f)
		.command(commands::skillgraph(), |f| f)
		.command(commands::rivalgraph(), |f| f)
		.command(commands::accuracygraph(), |f| f)
		.command(commands::quote(), |f| f)
		.command(commands::register(), |f| f)
		.command(commands::top(), |f| f)
		.command(commands::top10(), |f| f)
		.command(commands::aroundme(), |f| f)
		.command(commands::leaderboard(), |f| f)
		.command(commands::details(), |f| f)
		.command(commands::scoregraph(), |f| f)
		.run()
		.await?;

	Ok(())
}
