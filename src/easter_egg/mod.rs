// TODO: support simultaneous casing and char changes

mod casing;
use casing::*;
mod chars;
use chars::*;

use crate::{Context, Error, State};

/// Returns true if a funky invocation was detected and executed
pub async fn intercept_funky_invocation(error: &poise::FrameworkError<'_, State, Error>) -> bool {
	if let poise::FrameworkError::UnknownCommand {
		ctx,
		msg,
		prefix,
		invoked_command_name,
		args,
		framework,
		invocation_data,
		trigger,
	} = *error
	{
		let find_command = |s: &str| {
			framework
				.options
				.commands
				.iter()
				.find(|c| c.name.eq_ignore_ascii_case(s))
		};
		if let Some((command, transformer)) = char_transformer(invoked_command_name, find_command) {
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
		true
	} else {
		false
	}
}

pub fn reply_callback(ctx: Context<'_>, reply: &mut poise::CreateReply<'_>) {
	use futures::FutureExt as _;

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
		.invocation_data::<CharTransformer>()
		// We never lock it anywhere so we can do this
		.now_or_never()
		.flatten()
	{
		modify_strings(reply, &char_transformer.0);
	} else if let Some(casing_transformer) = casing_transformer(invoked_command_name) {
		modify_strings(reply, &casing_transformer);
	}
}
