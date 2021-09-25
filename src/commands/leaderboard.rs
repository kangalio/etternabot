//! All commands that show a leaderboard of sorts

use crate::{Context, Error};

#[derive(poise::SlashChoiceParameter)]
pub enum Skillset8 {
	#[name = "Overall"]
	Overall,
	#[name = "Stream"]
	Stream,
	#[name = "Jumpstream"]
	#[name = "JS"]
	Jumpstream,
	#[name = "Handstream"]
	#[name = "HS"]
	Handstream,
	#[name = "Stamina"]
	#[name = "Stam"]
	Stamina,
	#[name = "Jackspeed"]
	#[name = "Jacks"]
	#[name = "Jack"]
	Jackspeed,
	#[name = "Chordjack"]
	#[name = "CJ"]
	Chordjack,
	#[name = "Technical"]
	#[name = "Tech"]
	Technical,
}

impl From<Skillset8> for etterna::Skillset8 {
	fn from(x: Skillset8) -> Self {
		match x {
			Skillset8::Overall => etterna::Skillset8::Overall,
			Skillset8::Stream => etterna::Skillset8::Stream,
			Skillset8::Jumpstream => etterna::Skillset8::Jumpstream,
			Skillset8::Handstream => etterna::Skillset8::Handstream,
			Skillset8::Stamina => etterna::Skillset8::Stamina,
			Skillset8::Jackspeed => etterna::Skillset8::Jackspeed,
			Skillset8::Chordjack => etterna::Skillset8::Chordjack,
			Skillset8::Technical => etterna::Skillset8::Technical,
		}
	}
}

/// Retrieve leaderboard entries directly above and below the current user.
///
/// Call this command with `+aroundme [USERNAME] [SKILLSET] [AMOUNT]`
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn aroundme(
	ctx: Context<'_>,
	#[lazy]
	#[description = "EtternaOnline username"]
	username: Option<String>,
	#[description = "Skillset to sort by"] skillset: Option<Skillset8>,
	#[description = "How many entries to fetch above and below"] num_entries: Option<u32>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x.to_owned(),
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	let skillset = match skillset {
		Some(x) => x.into(),
		None => etterna::Skillset8::Overall,
	};

	let num_entries = num_entries.unwrap_or(7);

	let ranks = ctx.data().v1.user_ranks(&username).await?;
	let rank = ranks.get(skillset);

	let self_index = rank - 1; // E.g. first player in leaderboard has rank 1 but index 0;
	let entries = ctx
		.data()
		.web
		.leaderboard(
			self_index.saturating_sub(num_entries)..=(self_index + num_entries),
			etternaonline_api::web::LeaderboardSortBy::Rating(skillset),
			etternaonline_api::web::SortDirection::Descending,
		)
		.await
		.map_err(crate::no_such_user_or_skillset)?;

	// Detect if user doesn't exist (EO doesn't actually tell us this, it just returns garbage
	// results)
	let all_ones = etterna::UserRank {
		overall: 1,
		stream: 1,
		jumpstream: 1,
		handstream: 1,
		stamina: 1,
		jackspeed: 1,
		chordjack: 1,
		technical: 1,
	};
	let username_present_in_results = entries
		.iter()
		.any(|entry| entry.username.eq_ignore_ascii_case(&username));
	if ranks == all_ones && !username_present_in_results {
		return Err(crate::no_such_user_or_skillset(
			etternaonline_api::Error::UserNotFound {
				name: Some(username),
			},
		));
	}

	let self_entry = entries
		.iter()
		.find(|entry| entry.username.eq_ignore_ascii_case(&username))
		.or_else(|| entries.iter().find(|entry| entry.rank == rank)) // fallback 1
		.or_else(|| entries.get(0)) // fallback 2
		.ok_or("Error when retrieving leaderboard entries")?; // welp we did everything we could

	let mut output = String::from("```c\n");
	for entry in &entries {
		let is_self = std::ptr::eq(self_entry, entry);

		let flag_emoji = match &entry.country {
			Some(country) => crate::country_code_to_flag_emoji(&country.code) + " ",
			None => String::new(),
		};

		let diff_string_if_not_self = if is_self {
			String::from("       ")
		} else {
			format!(
				"({:+.02})",
				entry.rating.get(skillset) - self_entry.rating.get(skillset)
			)
		};

		output += &format!(
			"{prefix}#{rank} | {rating:.02} {diff} | {flag}{user}\n",
			prefix = if is_self { "> " } else { "  " },
			rank = entry.rank,
			rating = entry.rating.get(skillset),
			diff = diff_string_if_not_self,
			flag = flag_emoji,
			user = entry.username,
		);
	}
	output += "```";

	poise::send_reply(ctx, |f| {
		f.embed(|f| f.color(crate::ETTERNA_COLOR).description(output))
	})
	.await?;

	Ok(())
}

/// Get EtternaOnline leaderboards with an optional country code.
///
/// Call this command with `+leaderboard [COUNTRY CODE]`
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn leaderboard(
	ctx: Context<'_>,
	#[description = "Country code"] country: Option<String>,
) -> Result<(), Error> {
	let leaderboard = match &country {
		Some(country) => {
			let result = ctx.data().v1.country_leaderboard(country).await;
			if let Err(etternaonline_api::Error::NoUsersFound) = result {
				let response = format!("No users registered for country code `{}`", country);
				poise::say_reply(ctx, response).await?;
				return Ok(());
			}
			result?
		}
		None => ctx.data().v1.global_leaderboard().await?,
	};

	let title = match &country {
		Some(country) => format!(
			"{} Country leaderboard",
			crate::country_code_to_flag_emoji(country)
		),
		None => String::from("Worldwide leaderboard"),
	};

	let mut response = String::new();
	for (i, entry) in leaderboard.iter().enumerate() {
		response += &format!(
			"{0}. [{1}](https://etternaonline.com/user/{1}) ({2:.02})\n",
			i + 1,
			entry.username,
			entry.rating.overall,
			// can't use entry.user.country_code because that's always returned blank
		);
	}

	poise::send_reply(ctx, |f: &mut poise::CreateReply<'_>| {
		f.embed(|f| {
			f.title(title)
				.description(response)
				.color(crate::ETTERNA_COLOR)
		})
	})
	.await?;

	Ok(())
}
