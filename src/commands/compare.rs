//! All commands about comparing two profiles

use crate::{Context, Error};

/// Returns a string that may be shorter than `max_length`, but never longer
/// (measured in chars, not in bytes!)
fn gen_unicode_block_bar(max_length: usize, proportion: f32) -> String {
	// index x = x 8ths of a full block
	const BLOCK_CHARS: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

	let num_possible_steps = max_length * 8;
	let step = (proportion * num_possible_steps as f32) as usize;

	let num_full_blocks = step / 8;
	let type_of_last_block = step % 8;

	let mut string = String::with_capacity(max_length);
	for _ in 0..num_full_blocks {
		string.push(BLOCK_CHARS[8]);
	}
	// UNWRAP: due to the modulo the index is guaranteed to be in bounds
	string.push(*BLOCK_CHARS.get(type_of_last_block).unwrap());

	if let Some((truncation_point, _)) = string.char_indices().nth(max_length) {
		string.truncate(truncation_point);
	}

	string
}

/// Maps a value from src_range to dest_range. The value doesn't need to be inside src_range
///
/// ```rust
/// assert_eq!(map_range(15.0, 10.0..20.0, 3.0..4.0), 3.5);
/// assert_eq!(map_range(15.0, 10.0..20.0, -1.0, -3.0), -2.0);
/// assert_eq!(map_range(30.0, 10.0..20.0, -1.0, -3.0), -5.0);
/// ```
fn rescale(value: f32, src_range: std::ops::Range<f32>, dest_range: std::ops::Range<f32>) -> f32 {
	let proportion = (value - src_range.start) / (src_range.end - src_range.start);
	dest_range.start + proportion * (dest_range.end - dest_range.start)
}

async fn profile_compare(
	ctx: Context<'_>,
	me: &str,
	you: &str,
	expanded: bool,
) -> Result<(), Error> {
	let me = ctx.data().v1.user_data(me).await?;
	let you = ctx.data().v1.user_data(you).await?;

	let my_rating = &me.rating;
	let your_rating = &you.rating;

	let mut string = "```Prolog\n".to_owned();
	for skillset in etterna::Skillset8::iter() {
		string += &format!(
			"{: >10}:   {: >5.2}  {}  {: >5.2}   {:+.2}\n",
			skillset.to_string(), // to_string, or the padding won't work
			my_rating.get(skillset),
			if (my_rating.get(skillset) - your_rating.get(skillset)).abs() < f32::EPSILON {
				"="
			} else if my_rating.get(skillset) > your_rating.get(skillset) {
				">"
			} else {
				"<"
			},
			your_rating.get(skillset),
			my_rating.get(skillset) - your_rating.get(skillset),
		);
	}
	string += "```";

	let (mut min_ss_rating, mut max_ss_rating) = (f32::INFINITY, f32::NEG_INFINITY);
	for ss in etterna::Skillset8::iter() {
		let my_rating = my_rating.get(ss);
		let your_rating = your_rating.get(ss);
		if my_rating < min_ss_rating {
			min_ss_rating = my_rating;
		}
		if your_rating < min_ss_rating {
			min_ss_rating = your_rating;
		}
		if my_rating > max_ss_rating {
			max_ss_rating = my_rating;
		}
		if your_rating > max_ss_rating {
			max_ss_rating = your_rating;
		}
	}

	let bar_graph_block = if expanded {
		let mut bar_graph_block = "```prolog\n".to_owned();
		for skillset in etterna::Skillset8::iter() {
			let my_rating = my_rating.get(skillset);
			let your_rating = your_rating.get(skillset);
			bar_graph_block += &format!(
				"{: >10}:   \"░▒▓{}\"\n              “░▒▓{}“\n\n",
				skillset.to_string(), // to_string, or the padding won't work
				gen_unicode_block_bar(
					18,
					rescale(my_rating, min_ss_rating..max_ss_rating, 0.0..1.0)
				),
				gen_unicode_block_bar(
					18,
					rescale(your_rating, min_ss_rating..max_ss_rating, 0.0..1.0)
				),
			)
		}
		bar_graph_block += "```";
		Some(bar_graph_block)
	} else {
		None
	};

	poise::send_reply(ctx, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.title(format!(
					"{} {} vs. {} {}",
					crate::country_code_to_flag_emoji(&me.country_code.unwrap_or_default()),
					me.user_name,
					you.user_name,
					crate::country_code_to_flag_emoji(&you.country_code.unwrap_or_default()),
				))
				.description(string);

			if let Some(bar_graph_block) = bar_graph_block {
				e.field(
					format!("Above is {}, below is {}", me.user_name, you.user_name),
					bar_graph_block,
					false,
				);
			}

			e
		})
	})
	.await?;

	Ok(())
}

/// Compare your skillsets against your rival
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn rival(
	ctx: Context<'_>,
	#[description = "Show a bar chart of individual skillsets"]
	#[flag]
	expanded: bool,
) -> Result<(), Error> {
	let me = &ctx.data().get_eo_username(ctx.author()).await?;

	let rival = ctx
		.data()
		.lock_data()
		.rival(ctx.author().id)
		.map(|x| x.to_owned());
	let you = match rival {
		Some(rival) => rival,
		None => {
			poise::say_reply(ctx, "Set your rival first with `+rivalset USERNAME`").await?;
			return Ok(());
		}
	};

	profile_compare(ctx, me, &you, expanded).await
}

/// Compare two users' skillsets.
///
/// Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`. Add `expanded` at the end to see a graphic
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn compare(
	ctx: Context<'_>,
	#[description = "User on the left side of the comparison"]
	#[lazy]
	left: Option<String>,
	#[description = "User on the right side of the comparison"] right: String,
	#[description = "Show a bar chart of individual skillsets"]
	#[flag]
	expanded: bool,
) -> Result<(), Error> {
	let left = match left {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	profile_compare(ctx, &left, &right, expanded).await
}

/// Set a rival to compete against!
///
/// Call this command with `+rivalset YOUR_EO_USERNAME`
#[poise::command(prefix_command, aliases("setrival"), track_edits, slash_command)]
pub async fn rivalset(
	ctx: Context<'_>,
	#[description = "EtternaOnline username of your new rival"] rival: String,
) -> Result<(), Error> {
	if ctx.data().v1.user_data(&rival).await.is_err() {
		poise::say_reply(ctx, format!("User `{}` doesn't exist", rival)).await?;
		return Ok(());
	}

	let response = match ctx
		.data()
		.lock_data()
		.set_rival(ctx.author().id, rival.to_owned())
	{
		Some(old_rival) => format!(
			"Successfully updated your rival from `{}` to `{}`",
			old_rival, rival,
		),
		None => format!("Successfully set your rival to `{}`", rival),
	};
	poise::say_reply(ctx, response).await?;

	Ok(())
}
