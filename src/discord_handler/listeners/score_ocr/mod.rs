mod ocr;
pub use ocr::{
	Error as OcrError, EvaluationScreenData, MINIMUM_EQUALITY_SCORE_TO_BE_PROBABLY_EQUAL,
};

use super::State;

use crate::{serenity, Error};

struct Candidate {
	guild_id: serenity::GuildId,
	channel_id: serenity::ChannelId,
	message_id: serenity::MessageId,
	#[allow(dead_code)] // idk maybe we will need this again in the future
	author_id: serenity::UserId,

	scorekey: etterna::Scorekey,
	user_id: u32,

	reactors: std::collections::HashSet<serenity::User>,
	score_card_has_been_printed: bool,
}

pub struct ScoreCardTrigger<'a> {
	pub scorekey: &'a etterna::Scorekey,
	pub eo_user_id: u32,
	pub trigger_msg: (serenity::GuildId, serenity::ChannelId, serenity::MessageId),
}

pub struct OcrScoreCardManager {
	candidates: Vec<Candidate>,
}

impl OcrScoreCardManager {
	pub fn new() -> Self {
		Self { candidates: vec![] }
	}

	pub fn add_candidate(
		&mut self,
		guild_id: serenity::GuildId,
		channel_id: serenity::ChannelId,
		message_id: serenity::MessageId,
		author_id: serenity::UserId,
		scorekey: etterna::Scorekey,
		user_id: u32,
	) {
		println!(
			"Added new candidate {}, author id {}",
			&scorekey, author_id.0
		);
		self.candidates.push(Candidate {
			guild_id,
			channel_id,
			message_id,
			author_id,
			scorekey,
			user_id,

			reactors: std::collections::HashSet::new(),
			score_card_has_been_printed: false,
		});
	}

	/// Returns the score scorekey and user id if this reaction triggers the score card
	pub fn add_reaction(
		&mut self,
		ctx: &serenity::Context,
		reaction: &serenity::Reaction,
	) -> Result<Option<ScoreCardTrigger<'_>>, crate::Error> {
		// println!("Got reaction in score ocr card manager");

		// Let's check that the user even clicked the correct emoji type
		if reaction.emoji != serenity::ReactionType::Unicode("🔍".to_owned()) {
			return Ok(None);
		}

		// Find the Candidate that this reaction was made on, or return if the user made the
		// reaction on some unrelated message, i.e. a non-candidate
		let mut candidate = match self
			.candidates
			.iter_mut()
			.find(|c| c.message_id == reaction.message_id)
		{
			Some(candidate) => candidate,
			None => return Ok(None),
		};

		// If it has already been printed, stop. We don't want to print the card over and over
		// again
		if candidate.score_card_has_been_printed {
			println!("Has already been printed; skipping");
			return Ok(None);
		}

		println!(
			"Alright the reaction from <@{}> was legit; we now have {} reactions",
			reaction.user_id,
			candidate.reactors.len(),
		);
		candidate.reactors.insert(reaction.user(&ctx.http)?);

		Ok(if candidate.reactors.len() >= 2 {
			candidate.score_card_has_been_printed = true;
			Some(ScoreCardTrigger {
				scorekey: &candidate.scorekey,
				eo_user_id: candidate.user_id,
				trigger_msg: (
					candidate.guild_id,
					candidate.channel_id,
					candidate.message_id,
				),
			})
		} else {
			None
		})
	}
}

pub fn check_potential_score_screenshot(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
) -> Result<(), Error> {
	let guild_id = match msg.guild_id {
		Some(x) => x,
		None => return Ok(()), // this msg was sent in DMs
	};

	if msg.channel_id != state.config.score_channel {
		return Ok(());
	}

	let attachment = match msg.attachments.iter().find(|a| a.width.is_some()) {
		Some(a) => a,
		None => return Ok(()), // non-image post in score channel. Ignore
	};

	// sigh, I wish serenity had nice things, like methods built-in for this
	let member = super::get_guild_member(&ctx, &msg)?;

	if let Some(member) = member {
		// if was sent in a guild (as opposed to DMs)
		// If message was sent in EO and user doesn't have the appropriate role for the
		// score OCR feature, ignore this image
		if member.guild_id == state.config.etterna_online_guild_id {
			let has_required_role = member
				.roles
				.contains(&state.config.score_ocr_allowed_eo_role);
			if !has_required_role {
				return Ok(());
			}
		}
	}

	let bytes = attachment.download()?;
	println!("Post from {} on {:?}...", &msg.author.name, &msg.timestamp);
	let recognized = EvaluationScreenData::recognize_from_image_bytes(&bytes)?;
	println!("Recognized {:?}", recognized);

	let recognized_eo_username = recognized
		.iter()
		.filter_map(|r| r.eo_username.as_ref())
		.next();

	// If a username was recognized, try retrieve its user id. If the recognized username doesn't
	// exist, or no username was recognized in the first place, fall back to poster's saved
	// username
	let poster_eo_username = state.get_eo_username(&ctx, &msg)?;
	let user_id = match recognized_eo_username {
		Some(eo_username) => match state.web_session.user_details(&eo_username) {
			Ok(user_details) => user_details.user_id,
			Err(etternaonline_api::Error::UserNotFound) => {
				state.web_session.user_details(&poster_eo_username)?.user_id
			}
			Err(other) => return Err(other.into()),
		},
		None => state.web_session.user_details(&poster_eo_username)?.user_id,
	};

	let recent_scores = state.web_session.user_scores(
		user_id,
		0..50, // check recent scores for a match
		None,
		etternaonline_api::web::UserScoresSortBy::Date,
		etternaonline_api::web::SortDirection::Descending,
		true, // also search invalid
	)?;
	// println!("{:#?}", recent_scores);

	let mut best_equality_score_so_far = i32::MIN;
	let mut scorekey = None;
	for score in recent_scores.scores {
		let validity_dependant = match score.validity_dependant {
			Some(x) => x,
			None => continue, // don't check invalid scores (we don't have scorekey for those)
		};

		let score_as_eval = EvaluationScreenData {
			artist: None,
			eo_username: None, // no point comparing EO usernames - it's gonna match anyway
			judgements: Some(score.judgements.into()),
			song: Some(score.song_name),
			msd: None,
			ssr: Some(validity_dependant.ssr.overall),
			pack: None,
			rate: Some(score.rate),
			wifescore: Some(score.wifescore.as_percent()),
			difficulty: None,
			date: Some(score.date),
		};

		let mut best_equality_score = 0;
		let mut best_theme_i = 999;
		for (theme_i, recognized) in recognized.iter().enumerate() {
			// check results for all themes
			let equality_score = recognized.equality_score(&score_as_eval);
			if equality_score > best_equality_score {
				best_equality_score = equality_score;
				best_theme_i = theme_i;
			}
		}
		let equality_score = best_equality_score;
		let _theme_i = best_theme_i;
		// println!("Found match in theme {}", theme_i);

		if equality_score > MINIMUM_EQUALITY_SCORE_TO_BE_PROBABLY_EQUAL
			&& equality_score > best_equality_score_so_far
		{
			best_equality_score_so_far = equality_score;
			scorekey = Some(validity_dependant.scorekey);
		}
	}

	// Check if we actually found the matching score on EO
	let scorekey = match scorekey {
		Some(a) => a,
		None => return Ok(()),
	};

	msg.react(&ctx.http, '🔍')?;
	state.ocr_score_card_manager.lock().add_candidate(
		guild_id,
		msg.channel_id,
		msg.id,
		msg.author.id,
		scorekey,
		user_id,
	);

	Ok(())
}

pub fn reaction_add(
	state: &State,
	ctx: &serenity::Context,
	reaction: &serenity::Reaction,
) -> Result<(), Error> {
	if reaction.user_id == state.bot_user_id {
		return Ok(());
	}

	if let Some(score_info) = state
		.ocr_score_card_manager
		.lock()
		.add_reaction(ctx, reaction)?
	{
		// borrow checker headaches because this thing is monolithic
		let scorekey = score_info.scorekey.clone();
		let eo_user_id = score_info.eo_user_id;
		let trigger_msg = score_info.trigger_msg;

		super::send_score_card(
			state,
			ctx,
			trigger_msg.1,
			super::ScoreCard {
				scorekey: &scorekey,
				user_id: Some(eo_user_id),
				show_ssrs_and_judgements_and_modifiers: false,
				alternative_judge: None,
			},
		)?;
	}

	Ok(())
}
