//! This file sets up the framework, including configuration, registering all commands, registering
//! event handlers, global error handling, framework logging

use futures::FutureExt as _;

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

async fn listener(
	ctx: &serenity::Context,
	event: &poise::Event<'_>,
	framework: poise::FrameworkContext<'_, State, Error>,
	state: &State,
) -> Result<(), Error> {
	match event {
		poise::Event::Message { new_message } => {
			let invocation_data = tokio::sync::Mutex::new(Box::new(()) as _);
			let ctx = poise::PrefixContext {
				data: state,
				discord: ctx,
				msg: new_message,
				framework,
				// Just supply dummy values; we won't read these fields anyways
				command: &framework.options().commands[0],
				invoked_command_name: "",
				prefix: "",
				args: "",
				invocation_data: &invocation_data,
				action: |_| unreachable!(),
				trigger: poise::MessageDispatchTrigger::MessageCreate,
				__non_exhaustive: (), // 🎶 I - don't - care 🎶
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

async fn pre_command(ctx: Context<'_>) {
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

fn scream_back(ctx: Context<'_>, reply: &mut poise::CreateReply<'_>) {
	fn modify_strings(reply: &mut poise::CreateReply<'_>, f: &dyn Fn(&mut String)) {
		if let Some(s) = &mut reply.content {
			f(s);
		}
		for embed in &mut reply.embeds {
			if let Some(serde_json::Value::String(s)) = embed.0.get_mut("title") {
				f(s);
			}
			if let Some(serde_json::Value::String(s)) = embed.0.get_mut("description") {
				f(s);
			}
			if let Some(serde_json::Value::Object(author)) = embed.0.get_mut("author") {
				if let Some(serde_json::Value::String(s)) = author.get_mut("name") {
					f(s);
				}
			}
			if let Some(serde_json::Value::Array(fields)) = embed.0.get_mut("fields") {
				for field in fields {
					if let Some(serde_json::Value::String(s)) = field.get_mut("name") {
						f(s);
					}
					if let Some(serde_json::Value::String(s)) = field.get_mut("value") {
						f(s);
					}
				}
			}
		}
	}

	let invoked_command_name = ctx.invoked_command_name();
	// the dummy Context from the listener sets an empty string
	if invoked_command_name.is_empty() {
		return;
	}

	// If this was an "invalid" command invocation (reverse, or unicode chars), the transform
	// function is stored in invocation_data. Otherwise, for just silly casing, we check it
	// now and get the transformer for that
	if let Some(char_transformer) = ctx
		.invocation_data::<crate::easter_egg::CharTransformer>()
		// We never lock it anywhere so we can do this
		.now_or_never()
		.flatten()
	{
		modify_strings(reply, &char_transformer.0);
	} else if let Some(casing_transformer) =
		crate::easter_egg::casing_transformer(invoked_command_name)
	{
		modify_strings(reply, &casing_transformer);
	}
}

pub async fn run_framework(auth: crate::Auth, discord_bot_token: &str) -> Result<(), Error> {
	poise::Framework::builder()
		.user_data_setup(|ctx, _ready, _| Box::pin(State::load(ctx, auth)))
		.options(poise::FrameworkOptions {
			commands: vec![
				commands::compare(),
				commands::help(),
				commands::profile(),
				commands::pattern(),
				commands::ping(),
				commands::servers(),
				commands::uptime(),
				commands::lastsession(),
				commands::randomscore(),
				commands::lookup(),
				commands::scrollset(),
				commands::userset(),
				commands::rivalset(),
				commands::rs(),
				commands::rival(),
				commands::skillgraph(),
				commands::rivalgraph(),
				commands::accuracygraph(),
				commands::quote(),
				commands::register(),
				commands::top(),
				commands::top10(),
				commands::aroundme(),
				commands::leaderboard(),
				commands::details(),
				commands::scoregraph(),
			],
			listener: |ctx, event, framework, state| {
				Box::pin(listener(ctx, event, framework, state))
			},
			on_error: |error| {
				Box::pin(async move {
					if let poise::FrameworkError::UnknownCommand {
						ctx,
						msg,
						prefix,
						invoked_command_name,
						args,
						framework,
						invocation_data,
						trigger,
					} = error
					{
						let find_command = |s: &str| {
							framework
								.options
								.commands
								.iter()
								.find(|c| c.name.eq_ignore_ascii_case(s))
						};
						if let Some((command, transformer)) =
							crate::easter_egg::char_transformer(invoked_command_name, find_command)
						{
							if let Some(action) = command.prefix_action {
								// Store the transformer, to be retrieved in the reply callback
								*invocation_data.lock().await = Box::new(transformer);
								if let Err(e) = poise::run_invocation(poise::PrefixContext {
									discord: ctx,
									msg,
									prefix,
									invoked_command_name,
									args,
									framework,
									command,
									data: framework.user_data().await,
									invocation_data,
									trigger,
									action,
									__non_exhaustive: (),
								})
								.await
								{
									e.handle(&framework.options).await;
								}
							}
						}
					}

					if let Err(e) = poise::builtins::on_error(error).await {
						println!("Error while handling error: {}", e);
					}
				})
			},
			prefix_options: poise::PrefixFrameworkOptions {
				prefix: Some("+".into()),
				edit_tracker: Some(poise::EditTracker::for_timespan(
					std::time::Duration::from_secs(3600),
				)),
				..Default::default()
			},
			command_check: Some(|ctx| Box::pin(user_is_allowed_bot_interaction(ctx))),
			pre_command: |ctx| Box::pin(pre_command(ctx)),
			reply_callback: Some(scream_back),
			owners: std::iter::FromIterator::from_iter([serenity::UserId(472029906943868929)]),
			..Default::default()
		})
		.token(discord_bot_token)
		.intents(
			serenity::GatewayIntents::non_privileged()
				| serenity::GatewayIntents::GUILD_MEMBERS
				| serenity::GatewayIntents::GUILD_PRESENCES
				| serenity::GatewayIntents::MESSAGE_CONTENT,
		)
		.run()
		.await?;

	Ok(())
}
