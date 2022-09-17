// TODO: support simultaneous casing and char changes

mod casing;
mod reverse;
mod unicode;

use crate::{Context, Error, State};

#[derive(Default, Clone)]
struct Transformations {
	casing: Option<fn(&mut String)>,
	reverse: bool,
	unicode: Option<std::sync::Arc<dyn Fn(&mut String) + Send + Sync>>,
}

fn detect_transformations<C>(
	mut s: String,
	find_command: impl Fn(&str) -> Option<C>,
) -> Option<(Transformations, C)> {
	let mut transformations = Transformations::default();

	if let Some((detransformed, f)) = unicode::detect(&s) {
		s = detransformed;
		transformations.unicode = Some(f);
	}

	let command = if let Some((command, detransformed)) = reverse::detect(&s, &find_command) {
		s = detransformed;
		transformations.reverse = true;
		command
	} else {
		find_command(&s)?
	};

	transformations.casing = casing::detect(&s);

	Some((transformations, command))
}

fn apply_transformations(transformations: &Transformations, s: &mut String) {
	if let Some(f) = transformations.casing {
		f(s);
	}

	if transformations.reverse {
		*s = s.chars().rev().collect();
	}

	if let Some(f) = &transformations.unicode {
		f(s);
	}
}

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
		if let Some((transformations, command)) =
			detect_transformations(invoked_command_name.to_string(), find_command)
		{
			if let Some(action) = command.prefix_action {
				// Store the transformations, to be retrieved in the reply callback
				*invocation_data.lock().await = Box::new(transformations);
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

	// If more than casing was transformed, poise failed to dispatch the command and called
	// UnknownCommand, thereby invoking our custom dispatcher above which stores the detected
	// transformations
	let transformations = ctx
		.invocation_data::<Transformations>()
		// We never lock it anywhere so we can do this
		.now_or_never()
		.flatten()
		.map(|x| x.clone());
	// If there are no transformations stored, the command was dispatched via poise builtin
	// dispatch, so only casing could have been transformed
	let transformations = transformations.unwrap_or_else(|| Transformations {
		casing: casing::detect(invoked_command_name),
		reverse: false,
		unicode: None,
	});

	modify_strings(reply, &|s| apply_transformations(&transformations, s));
}
