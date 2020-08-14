mod config;
mod pattern_visualize;
mod replay_graph;
mod score_ocr;
mod draw_skill_graph;

use crate::serenity; // use my custom serenity prelude
use etternaonline_api as eo;
use config::{Config, Data};
use thiserror::Error;

const CMD_TOP_HELP: &str = "Call this command with `+top[NN] [USERNAME] [SKILLSET]` (both params optional)";
const CMD_COMPARE_HELP: &str = "Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`";
const CMD_USERSET_HELP: &str = "Call this command with `+userset YOUR_EO_USERNAME`";
const CMD_RIVALSET_HELP: &str = "Call this command with `+rivalset YOUR_EO_USERNAME`";
const CMD_SCROLLSET_HELP: &str = "Call this command with `+scrollset [down/up]`";

#[derive(Error, Debug)]
pub enum Error {
	#[error("Attempted to send an invalid Discord message. One or more fields were probably empty")]
	AttemptedToSendInvalidMessage,
	#[error("User {discord_username} not found on EO. Please manually specify your EtternaOnline \
		username with `+userset`")]
	CouldNotDeriveEoUsername { discord_username: String },

	#[error(transparent)]
	EoApiError(#[from] eo::Error),
	#[error(transparent)]
	SerenityError(#[from] serenity::Error),
	#[error(transparent)]
	PatternVisualizeError(#[from] pattern_visualize::Error),
	#[error("{0}")]
	ReplayGraphError(String),
	#[error("{0}")]
	SkillGraphError(String),
	#[error("Failed analyzing the score evaluation screenshot: {0:?}")]
	ScoreOcr(#[from] score_ocr::Error),
}

fn country_code_to_flag_emoji(country_code: &str) -> String {
	let regional_indicator_value_offset = '🇦' as u32 - 'a' as u32;
	country_code
		.to_lowercase()
		.chars()
		.map(|c| std::char::from_u32(c as u32 + regional_indicator_value_offset).unwrap_or(c))
		.collect()
}

pub struct State {
	config: Config,
	data: Data,
	v2_session: eo::v2::Session,
	web_session: eo::web::Session,
	pattern_visualizer: pattern_visualize::PatternVisualizer,
	user_id: serenity::UserId,
	analyzed_score_screenshot_messages: Vec<serenity::MessageId>,
	score_screenshot_scorekey_user_ids: std::collections::HashMap<serenity::MessageId, (eo::Scorekey, u32)>,
}

impl State {
	pub fn load(bot_user_id: serenity::UserId) -> Result<Self, Error> {
		let v2_session = eo::v2::Session::new_from_login(
			crate::auth::EO_USERNAME.to_owned(),
			crate::auth::EO_PASSWORD.to_owned(),
			crate::auth::EO_CLIENT_DATA.to_owned(),
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(30000)),
		)?;

		let web_session = eo::web::Session::new(
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(300000)), // yes five whole fucking minutes
		);

		Ok(State {
			v2_session,
			web_session,
			config: Config::load(),
			data: Data::load(),
			pattern_visualizer: pattern_visualize::PatternVisualizer::load()?,
			user_id: bot_user_id,
			analyzed_score_screenshot_messages: vec![],
			score_screenshot_scorekey_user_ids: std::collections::HashMap::new(),
		})
	}

	fn get_eo_username(&mut self,
		_ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<String, Error> {
		if let Some(eo_username) = self.data.eo_username(msg.author.id.0) {
			return Ok(eo_username.to_owned());
		}

		match self.v2_session.user_details(&msg.author.name) {
			Ok(_) => Ok(msg.author.name.to_owned()),
			Err(eo::Error::UserNotFound) => {
				Err(Error::CouldNotDeriveEoUsername { discord_username: msg.author.name.to_owned() })
			},
			Err(other) => Err(other.into()),
		}
	}

	fn top_scores(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
		mut limit: u32,
	) -> Result<(), Error> {
		if !(1..=30).contains(&limit) {
			msg.channel_id.say(&ctx.http, "Only limits up to 30 are supported")?;
			return Ok(());
		}

		let args: Vec<&str> = text.split_whitespace().collect();

		let skillset;
		let eo_username;
		if args.len() == 0 {
			skillset = None;
			eo_username = self.get_eo_username(ctx, msg)?;
		} else if args.len() == 1 {
			match eo::Skillset7::from_user_input(args[0]) {
				Some(parsed_skillset) => {
					skillset = Some(parsed_skillset);
					eo_username = self.get_eo_username(ctx, msg)?;
				},
				None => {
					skillset = None;
					eo_username = args[0].to_owned(); // to_owned not strictly needed
				},
			}
		} else if args.len() == 2 {
			skillset = match eo::Skillset7::from_user_input(args[0]) {
				Some(parsed_skillset) => Some(parsed_skillset),
				None => {
					msg.channel_id.say(
						&ctx.http,
						format!("Unrecognized skillset \"{}\"", args[0]))?;
					return Ok(());
				}
			};
			eo_username = args[1].to_owned(); // to_owned not strictly needed
		} else {
			msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			return Ok(());
		}

		// Download top scores
		let top_scores = match skillset {
			None => self.v2_session.user_top_10_scores(&eo_username),
			Some(skillset) => self.v2_session.user_top_skillset_scores(&eo_username, skillset, limit),
		};
		if let Err(eo::Error::UserNotFound) = top_scores {
			msg.channel_id.say(&ctx.http, format!("No such user or skillset \"{}\"", eo_username))?;
			return Ok(());
		}
		let top_scores = top_scores?;

		let country_code = self.v2_session.user_details(&eo_username)?.country_code;

		let mut response = String::from("```");
		for (i, entry) in top_scores.iter().enumerate() {
			response += &format!(
				"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
				i + 1,
				&entry.song_name,
				entry.rate,
				entry.ssr_overall,
				entry.wifescore.as_percent(),
			);
		}

		if limit != 10 && skillset == None {
			limit = 10;
			response += "(due to a bug in the EO v2 API, only 10 entries can be shown in Overall mode)";
		}
		
		response += "```";

		let title = match skillset {
			None => format!("{}'s Top {}", eo_username, limit),
			Some(skillset) => format!("{}'s Top {} {}", eo_username, limit, skillset),
		};

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
			.color(crate::ETTERNA_COLOR)
			.description(&response)
			.author(|a| a
				.name(title)
				.url(format!("https://etternaonline.com/user/profile/{}", eo_username))
				.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", country_code))
			)
		))?;

		Ok(())
	}

	fn latest_scores(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
	) -> Result<(), Error> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let latest_scores = self.v2_session.user_latest_scores(&eo_username)?;

		let country_code = self.v2_session.user_details(&eo_username)?.country_code;

		let mut response = String::from("```");
		for (i, entry) in latest_scores.iter().enumerate() {
			response += &format!(
				"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
				i + 1,
				&entry.song_name,
				entry.rate,
				entry.ssr_overall,
				entry.wifescore.as_percent(),
			);
		}
		response += "```";

		let title = format!("{}'s Last 10 Scores", eo_username);

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
			.color(crate::ETTERNA_COLOR)
			.description(&response)
			.author(|a| a
				.name(title)
				.url(format!("https://etternaonline.com/user/profile/{}", eo_username))
				.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", country_code))
			)
		))?;

		Ok(())
	}

	fn profile(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
	) -> Result<(), Error> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let details = match self.v2_session.user_details(&eo_username) {
			Ok(details) => details,
			Err(eo::Error::UserNotFound) => {
				msg.channel_id.say(
					&ctx.http,
					format!("User `{}` was not found (maybe run `+userset`)", eo_username),
				)?;
				return Ok(());
			},
			Err(e) => return Err(e.into()),
		};

		let ranks = self.v2_session.user_ranks_per_skillset(&eo_username)?;

		let mut title = eo_username.to_owned();
		if details.is_moderator {
			title += " (Mod)";
		}
		if details.is_patreon {
			title += " (Patron)";
		}

		let mut rating_string = "```Prolog\n".to_owned();
		for skillset in eo::Skillset8::iter() {
			rating_string += &format!(
				"{: >10}:   {: >5.2} (#{})\n",
				skillset.to_string(),
				details.rating.get(skillset),
				ranks.get(skillset),
			);
		}
		rating_string += "```";

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|embed| {
			embed
				.description(rating_string)
				.author(|a| a
					.name(&title)
					.url(format!("https://etternaonline.com/user/profile/{}", &eo_username))
					.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", &details.country_code))
				)
				.thumbnail(format!("https://etternaonline.com/avatars/{}", &details.avatar_url))
				.color(crate::ETTERNA_COLOR);
			if let Some(modifiers) = &details.default_modifiers {
				embed.field("Default modifiers:", modifiers, false);
			}
			if !details.about_me.is_empty() {
				embed.field(
					format!("About {}:", eo_username),
					html2md::parse_html(&details.about_me),
					false
				);
			}
			
			embed
		}
		))?;

		Ok(())
	}
	
	fn pattern(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		args: &str,
	) -> Result<(), Error> {
		let mut args: Vec<&str> = args.split_whitespace().collect();
		let mut arg_indices_to_remove = vec![];

		let mut interval_num_rows = 192 / 16;
		for (i, token) in args.iter().enumerate() {
			let ending = ["st", "sts", "nd", "nds", "th", "ths"].iter()
				.find(|&e| token.ends_with(e));
			let ending = match ending { Some(a) => a, None => continue };

			// at this point, this arg was surely intended to be a notes type arg, so we can already
			// remove it from the list of parsed arg indices. That's so that `+pattern 57ths 123`
			// doesn't generate as `5-7-1-2-3`
			arg_indices_to_remove.push(i);

			let note_type: usize = match token[..(token.len() - ending.len())].parse() {
				Ok(n) => n,
				Err(_) => continue,
			};
			if note_type == 0 {
				// early continue here to prevent crash through `192 % 0` operation
				continue;
			}
			if 192 % note_type != 0 { continue }

			interval_num_rows = 192 / note_type;
		}

		let mut scroll_type = None;
		for (i, arg) in args.iter().enumerate() {
			match arg.to_lowercase().as_str() {
				"up" => scroll_type = Some(pattern_visualize::ScrollType::Upscroll),
				"down" | "reverse" => scroll_type = Some(pattern_visualize::ScrollType::Downscroll),
				_ => continue,
			}
			arg_indices_to_remove.push(i);
		}
		let scroll_type = scroll_type.unwrap_or_else(||
			self.data.scroll(msg.author.id.0).unwrap_or(pattern_visualize::ScrollType::Upscroll)
		);

		// this is super fucking hacky
		let mut i = 0;
		args.retain(|_| (!arg_indices_to_remove.contains(&i), i += 1).0);
		let args_string = args.join("");

		let bytes = self.pattern_visualizer.generate(&args_string, scroll_type, interval_num_rows)?;

		// Send the image into the channel where the summoning message comes from
		msg.channel_id.send_files(&ctx.http, vec![(bytes.as_slice(), "output.png")], |m| m)?;

		Ok(())
	}

	fn profile_compare(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		me: &str,
		you: &str,
	) -> Result<(), Error> {
		let me = self.v2_session.user_details(&me)?;
		let you = self.v2_session.user_details(you)?;

		let mut string = "```Prolog\n".to_owned();
		for skillset in eo::Skillset8::iter() {
			string += &format!(
				"{: >10}:   {: >5.2}  {}  {: >5.2}   {:+.2}\n",
				skillset.to_string(), // to_string, or the padding won't work
				me.rating.get(skillset),
				if (me.rating.get(skillset) - you.rating.get(skillset)).abs() < f32::EPSILON {
					"="
				} else if me.rating.get(skillset) > you.rating.get(skillset) { 
					">"
				} else {
					"<"
				},
				you.rating.get(skillset),
				me.rating.get(skillset) - you.rating.get(skillset),
			);
		}
		string += "```";

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
			.title(format!(
				"{} {} vs. {} {}",
				country_code_to_flag_emoji(&me.country_code),
				me.username,
				you.username,
				country_code_to_flag_emoji(&you.country_code),
			))
			.description(string)
		))?;

		Ok(())
	}

	fn skillgraph(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		args: &str,
	) -> Result<(), Error> {
		let eo_username = if args.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			args.to_owned()
		};

		msg.channel_id.say(&ctx.http, format!("Requesting data for {} (this may take a while)", eo_username))?;
		let user_id = self.web_session.user_details(&eo_username)?.user_id;
		let scores = self.web_session.user_scores(
			user_id,
			..,
			None,
			eo::web::UserScoresSortBy::Date,
			eo::web::SortDirection::Ascending,
			false, // exclude invalid
			
		)?;

		let skill_timeline = etterna::skill_timeline(
			scores.scores.iter()
				.filter_map(|s| s
					.user_id_and_ssr
					.as_ref()
					.map(|u| (s.date.as_str(), u.nerfed_ssr()))
				)
				.filter(|(_date, ssr)| etterna::Skillset7::iter()
					.map(|ss| ssr.get(ss)).all(|x| x < 40.0)
				),
			true,
		);
		draw_skill_graph::draw_skill_graph(&skill_timeline, "output.png")
			.map_err(Error::SkillGraphError)?;

		msg.channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;

		Ok(())
	}

	fn command(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		text: &str
	) -> Result<(), Error> {
		if cmd.starts_with("top") {
			if let Ok(limit) = cmd[3..].parse() {
				self.top_scores(ctx, msg, text, limit)?;
			} else {
				msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			}
			return Ok(());
		}

		match cmd {
			"ping" => {
				msg.channel_id.say(&ctx.http, "Pong!")?;
			},
			"help" => {
				msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
					.description(self.data.make_description(&self.config.minanyms))
					.color(crate::ETTERNA_COLOR)
				))?;
			},
			"profile" => {
				self.profile(ctx, msg, text)?;
			},
			"lastsession" => {
				self.latest_scores(ctx, msg, text)?;
			},
			"pattern" => {
				self.pattern(ctx, msg, text)?;
			},
			"skillgraph" => {
				self.skillgraph(ctx, msg, text)?;
			}
			"quote" => {
				let quote = &self.config.quotes[rand::random::<usize>() % self.config.quotes.len()];
				let string = match &quote.source {
					Some(source) => format!("> {}\n~ {}", quote.quote, source),
					None => format!("> {}", quote.quote),
				};
				msg.channel_id.say(&ctx.http, &string)?;
			}
			"rs" => {
				let eo_username = match text {
					"" => self.get_eo_username(ctx, msg)?,
					username => username.to_owned(),
				};
				let latest_scores = self.v2_session.user_latest_scores(&eo_username)?;
				let user_id = self.web_session.user_details(&eo_username)?.user_id;
				self.score_card(ctx, msg, &latest_scores[0].scorekey, user_id)?;
			}
			"scrollset" => {
				let scroll = match &text.to_lowercase() as &str {
					"down" | "downscroll" => pattern_visualize::ScrollType::Downscroll,
					"up" | "upscroll" => pattern_visualize::ScrollType::Upscroll,
					"" => {
						msg.channel_id.say(&ctx.http, CMD_SCROLLSET_HELP)?;
						return Ok(());
					},
					_ => {
						msg.channel_id.say(&ctx.http, format!("No such scroll '{}'", text))?;
						return Ok(());
					},
				};
				self.data.set_scroll(msg.author.id.0, scroll);
				self.data.save();
				msg.channel_id.say(&ctx.http, &format!("Your scroll type is now {:?}", scroll))?;
			}
			"userset" => {
				if text.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_USERSET_HELP)?;
					return Ok(());
				}
				if let Err(e) = self.v2_session.user_details(text) {
					if let eo::Error::UserNotFound = e {
						msg.channel_id.say(&ctx.http, &format!("User `{}` doesn't exist", text))?;
						return Ok(());
					} else {
						return Err(e.into());
					}
				}
				
				let response = match self.data.set_eo_username(
					msg.author.id.0,
					text.to_owned()
				) {
					Some(old_eo_username) => format!(
						"Successfully updated username from `{}` to `{}`",
						old_eo_username,
						text,
					),
					None => format!("Successfully set username to `{}`", text),
				};
				msg.channel_id.say(&ctx.http, &response)?;
				self.data.save();
			},
			"rivalset" => {
				if text.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_RIVALSET_HELP)?;
					return Ok(());
				}
				if let Err(eo::Error::UserNotFound) = self.v2_session.user_details(text) {
					msg.channel_id.say(&ctx.http, &format!("User `{}` doesn't exist", text))?;
					return Ok(());
				}

				let response = match self.data.set_rival(
					msg.author.id.0,
					text.to_owned()
				) {
					Some(old_rival) => format!(
						"Successfully updated your rival from `{}` to `{}`",
						old_rival,
						text,
					),
					None => format!("Successfully set your rival to `{}`", text),
				};
				msg.channel_id.say(&ctx.http, &response)?;
				self.data.save();
			},
			"rival" => {
				let me = &self.get_eo_username(ctx, msg)?;
				let you = match self.data.rival(msg.author.id.0) {
					Some(rival) => rival.to_owned(),
					None => {
						msg.channel_id.say(&ctx.http, "Set your rival first with `+rivalset USERNAME`")?;
						return Ok(());
					}
				};
				self.profile_compare(ctx, msg, me, &you)?;
			}
			"compare" => {
				let args: Vec<&str> = text.split_whitespace().collect();

				let me;
				let you;
				if args.len() == 1 {
					me = self.get_eo_username(ctx, msg)?;
					you = args[0];
				} else if args.len() == 2 {
					me = args[0].to_owned();
					you = args[1];
				} else {
					msg.channel_id.say(&ctx.http, CMD_COMPARE_HELP)?;
					return Ok(());
				}

				self.profile_compare(ctx, msg, &me, you)?;
			}
			_ => {},
		}
		Ok(())
	}

	fn song_card(&mut self,
		_ctx: &serenity::Context,
		_msg: &serenity::Message,
		song_id: u32,
	) -> Result<(), Error> {
		println!("Argh I really _want_ to show song info for {}, but the EO v2 API doesn't expose \
			the required functions :(", song_id);
		Ok(())
	}

	fn score_card(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		scorekey: impl AsRef<str>,
		user_id: u32,
	) -> Result<(), Error> {
		let scorekey = scorekey.as_ref();

		let score = self.v2_session.score_data(&scorekey)?;

		let ssrs_string = format!(r#"
```nim
	  Wife: {:.2}%
 Max Combo: {}
   Overall: {:.2}
    Stream: {:.2}
   Stamina: {:.2}
Jumpstream: {:.2}
Handstream: {:.2}
     Jacks: {:.2}
 Chordjack: {:.2}
 Technical: {:.2}
```
			"#,
			score.wifescore.as_percent(),
			score.max_combo,
			score.ssr.overall(),
			score.ssr.stream,
			score.ssr.stamina,
			score.ssr.jumpstream,
			score.ssr.handstream,
			score.ssr.jackspeed,
			score.ssr.chordjack,
			score.ssr.technical,
		);
		let ssrs_string = ssrs_string.trim();

		let judgements_string = format!(r#"
```nim
    Marvelous: {}
      Perfect: {}
        Great: {}
         Good: {}
          Bad: {}
         Miss: {}
    Hit Mines: {}
   Held Holds: {}
Dropped Holds: {}
 Missed Holds: {}
```
			"#,
			score.judgements.marvelouses,
			score.judgements.perfects,
			score.judgements.greats,
			score.judgements.goods,
			score.judgements.bads,
			score.judgements.misses,
			score.judgements.hit_mines,
			score.judgements.held_holds,
			score.judgements.let_go_holds,
			score.judgements.missed_holds,
		);
		let judgements_string = judgements_string.trim();

		let description = format!(
			"https://etternaonline.com/score/view/{}{}\n```\n{}\n```",
			scorekey,
			user_id,
			score.modifiers,
		);

		struct ReplayAnalysis {
			replay_graph_path: &'static str,
			wife2_score: etterna::Wifescore,
			wife3_score: etterna::Wifescore,
			wife3_kang_system_score: etterna::Wifescore,
			fastest_finger_jackspeed: f32, // NPS, single finger
			fastest_nps: f32,
			longest_100_combo: u32,
			longest_marv_combo: u32,
			longest_perf_combo: u32,
			longest_combo: u32,
		}


		fn do_replay_analysis(score: &eo::v2::ScoreData) -> Option<Result<ReplayAnalysis, Error>> {
			use etterna::SimpleReplay;

			let replay = score.replay.as_ref()?;

			let r = replay_graph::generate_replay_graph(replay, "replay_graph.png").transpose()?;
			if let Err(e) = r {
				return Some(Err(Error::ReplayGraphError(e)))
			}
			
			// in the following, DONT scale find_fastest_note_subset results by rate - I only needed
			// to do that for etterna-graph where the note seconds where unscaled. EO's note seconds
			// _are_ scaled though.

			let (_note_seconds_lanes, hit_seconds_lanes) = replay.split_into_lanes()?;
			let mut max_finger_nps = 0.0;
			for hit_seconds in &hit_seconds_lanes {
				let this_fingers_max_nps = etterna::find_fastest_note_subset(hit_seconds, 20, 20).speed;

				if this_fingers_max_nps > max_finger_nps {
					max_finger_nps = this_fingers_max_nps;
				}
			}

			let (_note_seconds, hit_seconds) = replay.split_into_notes_and_hits()?;
			let fastest_nps = etterna::find_fastest_note_subset(&hit_seconds, 100, 100).speed;

			Some(Ok(ReplayAnalysis {
				replay_graph_path: "replay_graph.png",
				wife2_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife2>(
					replay,
					score.judgements.hit_mines,
					score.judgements.let_go_holds + score.judgements.missed_holds, // is this correct?
					&etterna::J4,
				)?,
				wife3_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife3>(
					replay,
					score.judgements.hit_mines,
					score.judgements.let_go_holds + score.judgements.missed_holds, // is this correct?
					&etterna::J4,
				)?,
				wife3_kang_system_score: eo::rescore::<etterna::MatchingScorer, etterna::Wife3>(
					replay,
					score.judgements.hit_mines,
					score.judgements.let_go_holds + score.judgements.missed_holds, // is this correct?
					&etterna::J4,
				)?,
				fastest_finger_jackspeed: max_finger_nps,
				fastest_nps,
				longest_100_combo: replay.longest_combo(|hit| hit.is_within_window(0.005)),
				longest_marv_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.marvelous_window)),
				longest_perf_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.perfect_window)),
				longest_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.great_window)),
			}))
		}

		let replay_analysis = do_replay_analysis(&score).transpose()?;

		msg.channel_id.send_message(&ctx.http, |m| {
			m.embed(|e| {
				e
					.color(crate::ETTERNA_COLOR)
					.author(|a| a
						.name(&score.song_name)
						.url(format!("https://etternaonline.com/song/view/{}", score.song_id))
						.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", score.user.country_code))
					)
					// .thumbnail(format!("https://etternaonline.com/avatars/{}", score.user.avatar)) // takes too much space
					.description(description)
					.field("SSRs", ssrs_string, true)
					.field("Judgements", judgements_string, true)
					.footer(|f| f
						.text(format!("Played by {}", &score.user.username))
						.icon_url(format!("https://etternaonline.com/avatars/{}", score.user.avatar))
					);
				
				if let Some(analysis) = &replay_analysis {
					e
						.attachment(analysis.replay_graph_path)
						.field("Scoring systems comparison", format!(
							"_Note: these calculated scores are slightly inaccurate_\n\
								**Wife2**: {:.2}%\n\
								**Wife3**: {:.2}%\n\
								**Wife3**: {:.2}% (no CB rushes)\n",
							analysis.wife2_score.as_percent(),
							analysis.wife3_score.as_percent(),
							analysis.wife3_kang_system_score.as_percent(),
						), false)
						.field("Tap speeds", format!(
							"Fastest jack over a course of 20 notes: {:.2} NPS\n\
								Fastest total NPS over a course of 100 notes: {:.2} NPS",
							analysis.fastest_finger_jackspeed,
							analysis.fastest_nps,
						), false)
						.field("Combos", format!(
							"Longest combo: {}\n\
								Longest perfect combo: {}\n\
								Longest marvelous combo: {}\n\
								Longest 100% combo: {}\n",
							analysis.longest_combo,
							analysis.longest_perf_combo,
							analysis.longest_marv_combo,
							analysis.longest_100_combo,
						), false);
				}

				e
			});
			if let Some(analysis) = &replay_analysis {
				m.add_file(analysis.replay_graph_path);
			}
			m
		})?;

		Ok(())
	}

	pub fn message(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<(), Error> {
		// Let's not do this, because if a non existing command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		// If the message is in etternaonline server, and not in an allowed channel, and not sent
		// by a person with the permission to manage the guild, don't process the command
		let user_is_allowed_bot_interaction = {
			if let (Some(guild_id), Some(guild_member)) = (msg.guild_id, msg.member(&ctx.cache)) {
				*guild_id.as_u64() != self.config.etterna_online_guild_id
					|| self.config.allowed_channels.contains(msg.channel_id.as_u64())
					|| guild_member.permissions(&ctx.cache)?.manage_guild()
			} else {
				println!("Failed to retrieve guild information.... is this worrisome?");
				// "true" should really every user be allowed bot usage everyhwere, just because we
				// failed to retrieve guild information? (probably; the alternative is completely
				// denying bot usage)
				true
			}
		};

		if msg.channel_id.0 == self.config.score_channel {
			self.check_potential_score_screenshot(ctx, msg)?;
		}

		if msg.channel_id.0 == self.config.work_in_progress_channel { // #work-in-progress
			let url_regex = regex::Regex::new(r"http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\(\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+").unwrap();
			let num_links = url_regex.find_iter(&msg.content).count();
			if num_links == 0 && msg.attachments.is_empty() {
				msg.delete(&ctx.http)?;
				let notice_msg = msg.channel_id.say(&ctx.http, format!(
					"Only links and attachments are allowed in this channel. For discussions use <#{}>",
					self.config.work_in_progress_discussion_channel),
				)?;
				std::thread::sleep(std::time::Duration::from_millis(5000));
				notice_msg.delete(&ctx.http)?;
				return Ok(());
			}
		}

		if user_is_allowed_bot_interaction {
			for captures in regex::Regex::new(r"https://etternaonline.com/score/view/(S\w{40})(\d+)")
				.unwrap()
				.captures_iter(&msg.content)
			{
				let scorekey = &captures[1];
				let user_id: u32 = captures[2].parse()
					.expect("this HAS to be a number as per the regex..?");
				
				if let Err(e) = self.score_card(&ctx, &msg, scorekey, user_id) {
					println!("Error while showing score card for {}: {}", scorekey, e);
				}
			}
	
			for captures in regex::Regex::new(r"https://etternaonline.com/song/view/(\d+)(#(\d+))?")
				.unwrap()
				.captures_iter(&msg.content)
			{
				let song_id = match captures[1].parse() {
					Ok(song_id) => song_id,
					Err(_) => continue, // this wasn't a valid song view url after all
				};
				if let Err(e) = self.song_card(&ctx, &msg, song_id) {
					println!("Error while showing song card for {}: {}", song_id, e);
				}
			}
		}

		if msg.content.starts_with('+') {
			let text = &msg.content[1..];

			// Split message into command part and parameter part
			let mut a = text.splitn(2, ' ');
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

	pub fn guild_member_update(&mut self,
		ctx: serenity::Context,
		old: Option<serenity::Member>,
		new: serenity::Member
	) -> Result<(), Error> {
		let old = match old { Some(a) => a, None => return Ok(()) };
		
		let guild = new.guild_id.to_partial_guild(&ctx.http)?;
		
		let get_guild_role = |guild_id| {
			if let Some(guild) = guild.roles.get(guild_id) {
				Some(guild.name.as_str())
			} else {
				println!("Couldn't find role {:?} in guild roles ({:?})... weird", guild_id, guild.roles);
				None
			}
		};

		let has_max_300_now = new.roles.iter()
			.any(|r| get_guild_role(r) == Some("MAX 300"));
		let had_max_300_previously = old.roles.iter()
			.any(|r| get_guild_role(r) == Some("MAX 300"));
		
		if has_max_300_now && !had_max_300_previously {
			ctx.http.get_channel(self.config.promotion_gratulations_channel)?
				.guild().unwrap().read()
				.say(
					&ctx.http,
					format!("Congrats on the promotion, <@{}>!", old.user_id()
				)
			)?;
		}

		Ok(())
	}

	pub fn check_potential_score_screenshot(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<(), Error> {
		let attachment = match msg.attachments.iter().find(|a| a.width.is_some()) {
			Some(a) => a,
			None => return Ok(()), // non-image post in #scores. ignore
		};

		let bytes = attachment.download()?;
		let recognized = score_ocr::EvaluationScreenData::recognize_from_image_bytes(&bytes)?;
		println!("Recognized: {:#?}", recognized);

		let recognized_eo_username = recognized.iter().filter_map(|r| r.eo_username.as_ref()).next();
		
		// If a username was recognized, try retrieve its user id. If the recognized username doesn't
		// exist, or no username was recognized in the first place, fall back to poster's saved
		// username
		let poster_eo_username = self.get_eo_username(&ctx, &msg)?;
		let user_id = match recognized_eo_username {
			Some(eo_username) => match self.web_session.user_details(&eo_username) {
				Ok(user_details) => user_details.user_id,
				Err(eo::Error::UserNotFound) => self.web_session.user_details(&poster_eo_username)?.user_id,
				Err(other) => return Err(other.into()),
			},
			None => self.web_session.user_details(&poster_eo_username)?.user_id,
		};

		let recent_scores = self.web_session.user_scores(
			user_id,
			0..50, // check recent scores for a match
			None,
			eo::web::UserScoresSortBy::Date,
			eo::web::SortDirection::Descending,
			true, // also search invalid
		)?;
		// println!("{:#?}", recent_scores);

		let mut best_equality_score_so_far = i32::MIN;
		let mut scorekey = None;
		for score in recent_scores.scores {
			let score_as_eval = score_ocr::EvaluationScreenData {
				artist: None,
				eo_username: None, // no point comparing EO usernames - it's gonna match anyway
				judgements: Some(score.judgements),
				song: Some(score.song_name),
				msd: None,
				ssr: score.user_id_and_ssr.map(|x| x.ssr.overall()),
				pack: None,
				rate: Some(score.rate),
				wifescore: Some(score.wifescore.as_percent()),
				difficulty: None,
				date: Some(score.date),
			};

			let mut best_equality_score = 0;
			let mut best_theme_i = 999;
			for (theme_i, recognized) in recognized.iter().enumerate() { // check results for all themes
				let equality_score = recognized.equality_score(&score_as_eval);
				if equality_score > best_equality_score {
					best_equality_score = equality_score;
					best_theme_i = theme_i;
				}
			}
			let equality_score = best_equality_score;
			let theme_i = best_theme_i;
			println!("Found match in theme {}", theme_i);

			if equality_score > score_ocr::MINIMUM_EQUALITY_SCORE_TO_BE_PROBABLY_EQUAL
				&& equality_score > best_equality_score_so_far
			{
				best_equality_score_so_far = equality_score;
				scorekey = Some(score.scorekey);
			}
		}

		// Check if we actually found the matching score on EO
		let scorekey = match scorekey {
			Some(a) => a,
			None => return Ok(()),
		};

		msg.react(&ctx.http, '🔍')?;
		self.score_screenshot_scorekey_user_ids.insert(msg.id, (scorekey, user_id));

		Ok(())
	}

	pub fn reaction_add(&mut self,
		ctx: serenity::Context,
		reaction: serenity::Reaction,
	) -> Result<(), Error> {
		if reaction.user_id == self.user_id {
			return Ok(());
		}

		if self.analyzed_score_screenshot_messages.contains(&reaction.message_id) {
			// we don't need to analyze and echo the results twice.
			// In particularly, we don't want users to be able to overload the server by spam
			// clicking the reaction button
			return Ok(());
		} else {
			self.analyzed_score_screenshot_messages.push(reaction.message_id);
		}

		let message = reaction.message(&ctx.http)?;

		// it only counts when the original author reacts
		if reaction.user_id != message.author.id {
			return Ok(())
		}

		let (scorekey, user_id) = match self.score_screenshot_scorekey_user_ids.get(&message.id) {
			Some(x) => x.to_owned(),
			None => return Ok(()),
		};
		
		self.score_card(&ctx, &message, &scorekey, user_id)?;

		Ok(())
	}
}