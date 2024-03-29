#![allow(clippy::type_complexity)]

// TODO: support simultaneous casing and char changes

mod casing;
mod order;
mod unicode;

use crate::{Context, Error, State};

// Utility function
fn map_words(s: &mut String, f: fn(&str) -> String) {
	#[allow(clippy::needless_collect)] // intermediate collect is needed to avoid mutating borrowed
	let word_spans = s
		.split(|c: char| !c.is_alphabetic())
		.filter(|&s| !s.is_empty())
		.map(|word| {
			let index = word.as_ptr() as usize - s.as_ptr() as usize;
			(index..index + word.len(), f(word))
		})
		.collect::<Vec<_>>();

	// Iterate in reverse so that shifting won't mess up the later indices
	for (span, transformed) in word_spans.into_iter().rev() {
		s.replace_range(span, &transformed);
	}
}

#[derive(Default, Clone)]
struct Transformations {
	order: Option<fn(&str) -> String>,
	casing: Option<fn(&mut String)>,
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

	transformations.casing = casing::detect(&s);
	s.make_ascii_lowercase(); // normalize for the order detection

	let command = if let Some((command, _detransformed, f)) = order::detect(&s, &find_command) {
		// s = detransformed;
		transformations.order = Some(f);
		command
	} else {
		find_command(&s)?
	};

	Some((transformations, command))
}

fn apply_transformations(transformations: &Transformations, s: &mut String) {
	if let Some(f) = transformations.order {
		map_words(s, f);
	}

	if let Some(f) = transformations.casing {
		f(s);
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
		msg_content,
		framework,
		invocation_data,
		trigger,
	} = *error
	{
		let (invoked_command_name, args) =
			msg_content.split_at(msg_content.find(' ').unwrap_or(msg_content.len()));

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
					serenity_context: ctx,
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
					parent_commands: &[],
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
		order: None,
		casing: casing::detect(invoked_command_name),
		unicode: None,
	});

	modify_strings(reply, &|s| apply_transformations(&transformations, s));
}
