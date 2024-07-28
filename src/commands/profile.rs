//! Commands related to viewing your profile

use crate::{Context, Error};
use std::borrow::Cow;

/// Save your EtternaOnline username in the bot
///
/// Call this command with `+userset YOUR_EO_USERNAME`
#[poise::command(prefix_command, aliases("setuser"), track_edits, slash_command)]
pub async fn userset(
	ctx: Context<'_>,
	#[description = "Your EtternaOnline username"] username: String,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let new_user_entry = crate::config::UserRegistryEntry {
		discord_id: ctx.author().id,
		discord_username: ctx.author().name.to_owned(),
		eo_id: ctx.data().web.user_details(&username).await?.user_id,
		eo_username: username.to_owned(),
		last_known_num_scores: None,
		last_rating: None,
	};

	let old_eo_username;
	{
		let author_id = ctx.author().id;
		let mut data = ctx.data().lock_data();
		match data
			.user_registry
			.iter_mut()
			.find(|u| u.discord_id == author_id)
		{
			Some(existing_user_entry) => {
				old_eo_username = Some(existing_user_entry.eo_username.clone());
				*existing_user_entry = new_user_entry;
			}
			None => {
				old_eo_username = None;
				data.user_registry.push(new_user_entry);
			}
		}
	}

	let response = match old_eo_username {
		Some(old) => format!(
			"Successfully updated username from `{}` to `{}`",
			old, username,
		),
		None => format!("Successfully set username to `{}`", username),
	};
	poise::say_reply(ctx, response).await?;

	Ok(())
}

fn truncate_text_maybe(text_body: &str, max_length: usize) -> Cow<'_, str> {
	let truncation_msg = "...";

	// check the char limit first, because otherwise we could produce a too large message
	if text_body.len() + truncation_msg.len() > max_length {
		// This is how long the text body may be at max to conform to Discord's limit
		let available_space = max_length - truncation_msg.len();

		let mut cut_off_point = available_space;
		while !text_body.is_char_boundary(cut_off_point) {
			cut_off_point -= 1;
		}

		Cow::Owned(format!("{}{}", &text_body[..cut_off_point], truncation_msg))
	} else {
		Cow::Borrowed(text_body)
	}
}

/// Display your skillsets and your improvements since last time
#[poise::command(prefix_command, aliases("advprof"), track_edits, slash_command)]
pub async fn profile(
	ctx: Context<'_>,
	#[description = "EtternaOnline username. If not specified, shows your stats"]
	#[autocomplete = "crate::autocomplete_username"]
	eo_username: Option<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let (eo_username, overwrite_prev_ratings) = match eo_username {
		Some(eo_username) => (eo_username, false),
		None => (ctx.data().get_eo_username(ctx.author()).await?, true),
	};

	let details = ctx.data().eo2.user(&eo_username).await?;
	let ranks = details.rank();

	let mut title = eo_username.to_owned();
	if details.roles.contains(&"admin".into()) {
		title += " (Admin)";
	}
	// This doesn't exist with EO2 yet
	if details.supporter {
		title += " (Patron)";
	}

	let (mut min_ss_rating, mut max_ss_rating) = (f32::INFINITY, f32::NEG_INFINITY);
	for ss in etterna::Skillset8::iter() {
		let ss_rating = details.skillsets.skillsets8().get(ss);
		if ss_rating < min_ss_rating {
			min_ss_rating = ss_rating;
		}
		if ss_rating > max_ss_rating {
			max_ss_rating = ss_rating;
		}
	}

	let rating_string = {
		let mut data = ctx.data().lock_data();
		// None if user is not in registry, Some(None) if user is in registry but no prev rating
		let previous_ratings = data
			.user_registry
			.iter_mut()
			.find(|entry| entry.eo_username.eq_ignore_ascii_case(&eo_username))
			.map(|entry| &mut entry.last_rating);

		let mut rating_string = "```prolog\n".to_owned();
		for skillset in etterna::Skillset8::iter() {
			match &previous_ratings {
				Some(Some(prev)) => {
					rating_string += &format!(
						"{: >10}:   {: >5.2} ({: >+4.2})  #{: <4}\n",
						skillset.to_string(),
						details.skillsets.skillsets8().get(skillset),
						details.skillsets.skillsets8().get(skillset) - prev.get(skillset),
						ranks.get(skillset),
					)
				}
				Some(None) | None => {
					rating_string += &format!(
						"{: >10}:   {: >5.2}  #{: <4}\n",
						skillset.to_string(),
						details.skillsets.skillsets8().get(skillset),
						ranks.get(skillset),
					)
				}
			}
		}
		rating_string += "```";

		if overwrite_prev_ratings {
			// TODO: could create new entry if doesn't already exist to store ratings
			if let Some(previous_ratings) = previous_ratings {
				*previous_ratings = Some(details.skillsets.skillsets8().clone());
			}
		}

		rating_string
	};

	poise::send_reply(ctx, |m| {
		m.embed(|embed| {
			embed
				.description(rating_string)
				.author(|a| {
					a.name(title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							&eo_username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							Some(&*details.country).as_deref().unwrap_or("")
						))
				})
				.thumbnail(format!(
					"https://etternaonline.com/avatars/{}",
					&details.avatar
				))
				.color(crate::ETTERNA_COLOR);
			if let Some(about_me) = Some(&details.bio) {
				let about_me = html2md::parse_html(about_me);
				if !about_me.is_empty() {
					embed.field(
						format!("About {}:", eo_username),
						truncate_text_maybe(&about_me, 1024),
						false,
					);
				}
			}

			embed
		})
	})
	.await?;

	Ok(())
}
