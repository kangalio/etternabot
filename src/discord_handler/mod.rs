mod config;
mod replay_graph;
mod draw_skill_graph;

use crate::serenity; // use my custom serenity prelude
use etternaonline_api as eo;
use config::{Config, Data};
use thiserror::Error;
use rand::Rng as _;

const CMD_TOP_HELP: &str = "Call this command with `+top[NN] [USERNAME] [SKILLSET]` (both params optional)";
const CMD_COMPARE_HELP: &str = "Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`. Add `expanded` at the end to see a graphic";
const CMD_USERSET_HELP: &str = "Call this command with `+userset YOUR_EO_USERNAME`";
const CMD_RIVALSET_HELP: &str = "Call this command with `+rivalset YOUR_EO_USERNAME`";
const CMD_SCROLLSET_HELP: &str = "Call this command with `+scrollset [down/up]`";
const CMD_RS_HELP: &str = "Call this command with `+rs [username] [judge]`";
const CMD_LOOKUP_HELP: &str = "Call this command with `+lookup DISCORDUSERNAME`";

// UNWRAP: those are valid regexes
static SCORE_LINK_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
	regex::Regex::new(r"https://etternaonline.com/score/view/(S\w{40})(\d+)").unwrap()
});
static LINK_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
	regex::Regex::new(r"http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\(\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+").unwrap()
});
static SONG_LINK_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
	regex::Regex::new(r"https://etternaonline.com/song/view/(\d+)(#(\d+))?").unwrap()
});
static JUDGE_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
	regex::Regex::new(r"[jJ](\d)").unwrap()
});

#[derive(Error, Debug)]
pub enum Error {
	#[error("Attempted to send an invalid Discord message. One or more fields were probably empty")]
	AttemptedToSendInvalidMessage,
	#[error("User {discord_username} not found on EO. Please manually specify your EtternaOnline \
		username with `+userset`")]
	CouldNotDeriveEoUsername { discord_username: String },
	#[error("EtternaOnline error: {0}")]
	EoApiError(#[from] eo::Error),
	#[error("Can't complete this request because EO login failed ({0})")]
	FailedEoLogin(eo::Error),
	#[error(transparent)]
	SerenityError(#[from] serenity::Error),
	#[error(transparent)]
	PatternVisualizeError(#[from] pattern_draw::Error),
	#[error("Failed parsing the pattern: {0}")]
	PatternParseError(#[from] pattern_draw::PatternParseError),
	#[error("{0}")]
	ReplayGraphError(String),
	#[error("{0}")]
	SkillGraphError(String),
	#[error("Failed analyzing the score evaluation screenshot: {0:?}")]
	ScoreOcr(#[from] score_ocr::Error),
	#[error("A score was requested from EO but none was sent")]
	NoScoreEvenThoughOneWasRequested,
	#[error("User not found in registry (`+userset` must have been called at least once)")]
	UserNotInRegistry,
}

fn country_code_to_flag_emoji(country_code: &str) -> Option<String> {
	if country_code.chars().any(|c| !c.is_alphabetic()) {
		return None;
	}

	let regional_indicator_value_offset = '🇦' as u32 - 'a' as u32;
	country_code
		.chars()
		.map(|c| std::char::from_u32(c.to_ascii_lowercase() as u32 + regional_indicator_value_offset))
		.collect()
}

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

fn get_random_score(
	registry_entry: &mut config::UserRegistryEntry,
	web_session: &eo::web::Session,
) -> Result<eo::web::UserScore, Error> {
	let scores = if let Some(last_known_num_scores) = registry_entry.last_known_num_scores {
		// choose a random score
		let score_index = rand::thread_rng().gen_range(0, last_known_num_scores);

		web_session.user_scores(
			registry_entry.eo_id,
			score_index..=score_index,
			None,
			etternaonline_api::web::UserScoresSortBy::Date, // doesnt matter
			etternaonline_api::web::SortDirection::Ascending, // doesnt matter
			true,
		)?
	} else {
		// let's get the first score by scorekey - the scorekey is pretty random, so this will seem
		// sufficiently random - at least for the first time. Doing it multiple times would yield
		// the same score every time BUT since we're writing the number of scores after this, future
		// invocations can directly request a random index
		web_session.user_scores(
			registry_entry.eo_id,
			0..1,
			None,
			etternaonline_api::web::UserScoresSortBy::Scorekey,
			etternaonline_api::web::SortDirection::Ascending,
			true,
		)?
	};

	registry_entry.last_known_num_scores = Some(scores.entries_before_search_filtering);

	scores.scores.into_iter().next().ok_or(Error::NoScoreEvenThoughOneWasRequested)
}

fn extract_judge_from_string(string: &str) -> Option<&etterna::Judge> {
	JUDGE_REGEX.captures_iter(string)
		.filter_map(|groups| {
			// UNWRAP: the regex definition contains a group
			let judge_num_string = groups.get(1).unwrap().as_str();

			let judge_num: u32 = judge_num_string.parse().ok()?;

			match judge_num {
				1 => Some(etterna::J1),
				2 => Some(etterna::J2),
				3 => Some(etterna::J3),
				4 => Some(etterna::J4),
				5 => Some(etterna::J5),
				6 => Some(etterna::J6),
				7 => Some(etterna::J7),
				8 => Some(etterna::J8),
				9 => Some(etterna::J9),
				_ => None,
			}
		})
		.next()
}

// Returns None if msg was sent in DMs
fn get_guild_member(
	ctx: &serenity::Context,
	msg: &serenity::Message
) -> Result<Option<serenity::Member>, serenity::Error> {
	Ok(match msg.guild_id {
		Some(guild_id) => Some(match msg.member(&ctx.cache) {
			Some(cached_member) => cached_member,
			None => ctx.http.get_member(guild_id.0, msg.author.id.0)?,
		}),
		None => None,
	})
}

struct ScoreCard<'a> {
	scorekey: &'a etterna::Scorekey,
	user_id: Option<u32>, // pass None if score link shouldn't be shown
	show_ssrs_and_judgements_and_modifiers: bool,
	alternative_judge: Option<&'a etterna::Judge>,
	#[allow(clippy::type_complexity)]
	triggerers: Option<(&'a [serenity::User], (serenity::GuildId, serenity::ChannelId, serenity::MessageId))>,
}

struct NoteskinProvider {
	dbz: pattern_draw::Noteskin,
	lambda: pattern_draw::Noteskin,
	wafles: pattern_draw::Noteskin,
	delta_note: pattern_draw::Noteskin,
	sbz: pattern_draw::Noteskin,
	mbz: pattern_draw::Noteskin,
	eo_baner: pattern_draw::Noteskin,
	rustmania: pattern_draw::Noteskin,
}

/// The contained Option must be Some!!!
struct IdkWhatImDoing<'a> {
	guard: crate::mutex::MutexGuard<'a, Option<eo::v2::Session>>,
}
impl std::ops::Deref for IdkWhatImDoing<'_> {
	type Target = eo::v2::Session;

	fn deref(&self) -> &Self::Target {
		// UNWRAP: this will work because it's an invariant of this type
		self.guard.as_ref().unwrap()
	}
}

struct AutoSaveGuard<'a> {
	guard: crate::mutex::MutexGuard<'a, Data>,
}
impl std::ops::Deref for AutoSaveGuard<'_> {
	type Target = Data;

	fn deref(&self) -> &Self::Target {
		&*self.guard
	}
}
impl std::ops::DerefMut for AutoSaveGuard<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut *self.guard
	}
}
impl Drop for AutoSaveGuard<'_> {
	fn drop(&mut self) {
		self.guard.save();
	}
}

pub struct State {
	start_time: std::time::Instant,
	config: Config,
	_data: crate::mutex::Mutex<Data>,
	v2_session: crate::mutex::Mutex<Option<eo::v2::Session>>, // stores the session, or None if login failed
	web_session: eo::web::Session,
	noteskin_provider: NoteskinProvider,
	user_id: serenity::UserId,
	ocr_score_card_manager: crate::mutex::Mutex<OcrScoreCardManager>,
}

impl State {
	pub fn load(ctx: &serenity::Context, bot_user_id: serenity::UserId) -> Result<Self, Error> {
		let web_session = eo::web::Session::new(
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(300_000)), // yes five whole fucking minutes
		);

		let config = Config::load();
		if ctx.http.get_channel(config.promotion_gratulations_channel)?.guild().is_none() {
			panic!("Configured promotion gratulations channel is not a valid guild channel!");
		}

		Ok(Self {
			start_time: std::time::Instant::now(),
			v2_session: crate::mutex::Mutex::new(match Self::attempt_v2_login() {
				Ok(v2) => Some(v2),
				Err(e) => {
					println!("Failed to login to EO on bot startup: {}. Continuing with no v2 session active", e);
					None
				}
			}),
			web_session,
			config,
			_data: crate::mutex::Mutex::new(Data::load()),
			user_id: bot_user_id,
			ocr_score_card_manager: crate::mutex::Mutex::new(OcrScoreCardManager::new()),
			noteskin_provider: NoteskinProvider {
				dbz: pattern_draw::Noteskin::read_ldur_with_6k(
					64,
					"assets/noteskin/dbz-notes.png", "assets/noteskin/dbz-receptor.png",
					"assets/noteskin/dbz-mine.png",
				)?,
				delta_note: pattern_draw::Noteskin::read_pump(
					64,
					"assets/noteskin/deltanote-center-notes.png", "assets/noteskin/deltanote-center-receptor.png",
					"assets/noteskin/deltanote-corner-notes.png", "assets/noteskin/deltanote-corner-receptor.png",
					"assets/noteskin/deltanote-mine.png",
				)?,
				sbz: pattern_draw::Noteskin::read_bar(
					64,
					"assets/noteskin/sbz-notes.png", "assets/noteskin/sbz-receptor.png",
					"assets/noteskin/dbz-mine.png",
				)?,
				mbz: pattern_draw::Noteskin::read_bar(
					64,
					"assets/noteskin/mbz-notes.png", "assets/noteskin/mbz-receptor.png",
					"assets/noteskin/dbz-mine.png",
				)?,
				lambda: {
					let mut lambda = pattern_draw::Noteskin::read_ldur_with_6k(
						128,
						"assets/noteskin/lambda-notes.png", "assets/noteskin/lambda-receptor.png",
						"assets/noteskin/lambda-mine.png",
					)?;
					lambda.resize_sprites(64);
					lambda
				},
				wafles: pattern_draw::Noteskin::read_ldur_with_6k(
					64,
					"assets/noteskin/wafles-notes.png", "assets/noteskin/wafles-receptor.png",
					"assets/noteskin/wafles-mine.png",
				)?,
				eo_baner: pattern_draw::Noteskin::read_ldur(
					120,
					"assets/noteskin/eobaner-note-left.png", "assets/noteskin/eobaner-receptor-left.png",
					"assets/noteskin/eobaner-note-down.png", "assets/noteskin/eobaner-receptor-down.png",
					"assets/noteskin/eobaner-note-up.png", "assets/noteskin/eobaner-receptor-up.png",
					"assets/noteskin/eobaner-note-right.png", "assets/noteskin/eobaner-receptor-right.png",
					"assets/noteskin/eobaner-mine.png",
				)?,
				rustmania: {
					let mut rustmania = pattern_draw::Noteskin::read_ldur_with_6k(
						224,
						"assets/noteskin/rustmania-notes.png", "assets/noteskin/rustmania-receptor.png",
						"assets/noteskin/rustmania-mine.png",
					)?;
					rustmania.turn_sprites_upside_down(); // I made an oopsie in gimp
					rustmania
				},
			},
		})
	}

	fn attempt_v2_login() -> Result<eo::v2::Session, eo::Error> {
		eo::v2::Session::new_from_login(
			crate::auth::EO_USERNAME.to_owned(),
			crate::auth::EO_PASSWORD.to_owned(),
			crate::auth::EO_CLIENT_DATA.to_owned(),
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(30000)),
		)
	}

	// Automatically saves when the returned guard goes out of scope
	fn lock_data(&self) -> AutoSaveGuard {
		AutoSaveGuard {
			guard: self._data.lock(),
		}
	}

	/// attempt to retrieve the v2 session object. If there is none because login had failed,
	/// retry login just to make sure that EO is _really_ done
	/// the returned value contains a mutex guard. so if thread 1 calls v2() while thread 2 still
	/// holds the result from its call to v2(), thread 1 will block.
	fn v2(&self) -> Result<IdkWhatImDoing, Error> {
		let mut v2_session = self.v2_session.lock();

		if v2_session.is_some() {
			Ok(IdkWhatImDoing { guard: v2_session })
		} else {
			match Self::attempt_v2_login() {
				Ok(v2) => {
					*v2_session = Some(v2);
					Ok(IdkWhatImDoing { guard: v2_session })
				},
				Err(e) => {
					*v2_session = None;
					Err(Error::FailedEoLogin(e))
				}
			}
		}
	}

	fn get_eo_username(&self,
		_ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<String, Error> {
		if let Some(user_entry) = self.lock_data().user_registry.iter()
			.find(|user| user.discord_id == msg.author.id.0)
		{
			return Ok(user_entry.eo_username.to_owned());
		}

		match self.v2()?.user_details(&msg.author.name) {
			Ok(_) => {
				// seems like the user's EO name is the same as their Discord name :)
				Ok(msg.author.name.to_owned())
			},
			Err(eo::Error::UserNotFound) => {
				Err(Error::CouldNotDeriveEoUsername { discord_username: msg.author.name.to_owned() })
			},
			Err(other) => Err(other.into()),
		}
	}

	fn get_eo_user_id(&self, eo_username: &str) -> Result<u32, Error> {
		match self.lock_data().user_registry.iter().find(|user| user.eo_username == eo_username) {
			Some(user) => Ok(user.eo_id),
			None => Ok(self.web_session.user_details(eo_username)?.user_id),
		}
	}

	/// The returned string must be displayed in an embed
	fn make_help_message(&self, pattern_help: bool) -> String {
		if pattern_help {
			r#"
**+pattern [down/up] [NNths] [noteskin] [zoom] [keymode] PATTERN STRING**
- `down/up` configures the scroll direction (note: you can configure your preferred scroll direction with `+scrollset`)
- `NNths` sets the note snap. This can be placed throughout the pattern string to change the snap mid-pattern. Can even be something like 20ths or 57ths!
- `noteskin` can be `delta-note`, `sbz`/`subtract-by-zero`, `dbz`/`divide-by-zero`, `mbz`/`multiply-by-zero`, `lambda`, or `wafles`/`wafles3`[.](https://pastebin.com/raw/5We1buQU) If omitted, a default will be chosen
- `zoom` applies a certain stretch to the notes
- `keymode` can be used to force a certain keymode when it's not obvious

To draw a chord, enclose the notes in bracketes: `[12][34][12][34]` creates a jumptrill.
Empty rows are written with `0` or `[]`.
Lane numbers beyond 9 must be enclosed in paranthesis: `123456789(10)` instead of `12345678910`.
Insert `M` to switch to mine mode for the current note row.

Examples:
`+pattern [13]4[32]1[24]1[23]4` draws a simple jumpstream
`+pattern 232421212423212` draws a runningman
`+pattern 2x 12ths 123432 16ths 1313` draws a few 12ths notes, followed by a 16ths trill, all stretched by a factor of 2
`+pattern 57ths 123432123412341234123` creates funny colors
`+pattern 6k [34]52[34]25` draws a pattern in 6k mode, even though the notes span across just 5 lanes
			"#.to_owned()
		} else {
			format!(
				r#"
Here are my commands: (Descriptions by Fission)

**+profile [username]**
*Show your fabulously superberful profile*
**+top10 [username] [skillset]**
*For when top9 isn't enough*
**+top[nn] [username] [skillset]**
*Sometimes we take things too far*
**+compare [user1] [user2]**
*One person is an objectively better person than the other, find out which one!*
**+rival**/**+rival expanded**
*But are you an objectively better person than gary oak?*
**+rivalgraph**

**+rivalset [username]**
*Replace gary oak with a more suitable rival*
**+userset [username]**
*Don't you dare set your user to* {} *you imposter*

More commands:
**+pattern [pattern string]**
*Visualize note patterns, for example `lrlr` or `[14]3[12]`. This command has many options, type `+help pattern` for that*
**+scrollset [down/up]**
*Set your preferred scroll type that will be used as a default*
**+skillgraph [user] [user 2] [...]**
*Show a graph of your profile rating over time, including all skillsets*
**+rs [username] [judge]**
*Show your most recent score*
**+quote**
*Print one of various random quotes, phrases and memes from various rhythm gaming communities*
**+lastsession [username]**
*Show the last 10 scores*
**+help**
*Print this message*

You can also post links to scores and I will show info about them. If you add a judge (e.g. "J7") to
your message, I will also show the wifescores with that judge.
				"#,
				// UNWRAP: as per gen_range docs the index is always below the vector length
				&self.config.minanyms.get(
					rand::thread_rng().gen_range(0, self.config.minanyms.len())
				).unwrap(),
			)
		}
	}

	fn top_scores(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		args: &str,
		mut limit: u32,
	) -> Result<(), Error> {
		if !(1..=30).contains(&limit) {
			msg.channel_id.say(&ctx.http, "Only limits up to 30 are supported")?;
			return Ok(());
		}

		let args: Vec<&str> = args.split_whitespace().collect();

		let skillset;
		let eo_username;
		match *args.as_slice() {
			[] => {
				skillset = None;
				eo_username = self.get_eo_username(ctx, msg)?;
			},
			[skillset_or_username] => {
				match etterna::Skillset7::from_user_input(skillset_or_username) {
					Some(parsed_skillset) => {
						skillset = Some(parsed_skillset);
						eo_username = self.get_eo_username(ctx, msg)?;
					},
					None => {
						skillset = None;
						eo_username = skillset_or_username.to_owned();
					},
				}
			},
			[skillset_str, username] => {
				skillset = match etterna::Skillset7::from_user_input(skillset_str) {
					Some(parsed_skillset) => Some(parsed_skillset),
					None => {
						msg.channel_id.say(
							&ctx.http,
							format!("Unrecognized skillset \"{}\"", username))?;
						return Ok(());
					}
				};
				eo_username = username.to_owned();
			},
			_ => {
				msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
				return Ok(());
			}
		}

		// Download top scores
		let top_scores = match skillset {
			None => self.v2()?.user_top_10_scores(&eo_username),
			Some(skillset) => self.v2()?.user_top_skillset_scores(&eo_username, skillset, limit),
		};
		if let Err(eo::Error::UserNotFound) = top_scores {
			msg.channel_id.say(&ctx.http, format!("No such user or skillset \"{}\"", eo_username))?;
			return Ok(());
		}
		let top_scores = top_scores?;

		let country_code = self.v2()?.user_details(&eo_username)?.country_code;

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
				.icon_url(format!("https://etternaonline.com/img/flags/{}.png", country_code))
			)
		))?;

		Ok(())
	}

	fn latest_scores(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
	) -> Result<(), Error> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let latest_scores = self.v2()?.user_latest_scores(&eo_username)?;

		let country_code = self.v2()?.user_details(&eo_username)?.country_code;

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
				.icon_url(format!("https://etternaonline.com/img/flags/{}.png", country_code))
			)
		))?;

		Ok(())
	}

	fn profile(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
	) -> Result<(), Error> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let details = self.v2()?.user_details(&eo_username)?;
		let ranks = self.v2()?.user_ranks_per_skillset(&eo_username)?;

		let mut title = eo_username.to_owned();
		if details.is_moderator {
			title += " (Mod)";
		}
		if details.is_patreon {
			title += " (Patron)";
		}

		let rating = details.rating.calc_player_overall_pre_070();
		
		let (mut min_ss_rating, mut max_ss_rating) = (f32::INFINITY, f32::NEG_INFINITY);
		for ss in etterna::Skillset8::iter() {
			let ss_rating = rating.get(ss);
			if ss_rating < min_ss_rating { min_ss_rating = ss_rating; }
			if ss_rating > max_ss_rating { max_ss_rating = ss_rating; }
		}
		
		let mut rating_string = "```prolog\n".to_owned();
		for skillset in etterna::Skillset8::iter() {
			let ss_rating = rating.get(skillset);
			rating_string += &format!(
				"{: >10}:   {: >5.2}  #{: <4} ░▒▓{}\n",
				skillset.to_string(),
				ss_rating,
				ranks.get(skillset),
				gen_unicode_block_bar(7, rescale(ss_rating, min_ss_rating..max_ss_rating, 0.0..1.0)),
			);
		}
		rating_string += "```";

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|embed| {
			embed
				.description(rating_string)
				.author(|a| a
					.name(&title)
					.url(format!("https://etternaonline.com/user/profile/{}", &eo_username))
					.icon_url(format!("https://etternaonline.com/img/flags/{}.png", &details.country_code))
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
	
	fn pattern(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		args: &str,
	) -> Result<(), Error> {
		let mut noteskin_override = None;
		let mut keymode_override = None;
		let mut snap = etterna::Snap::_16th.into();
		let mut vertical_spacing_multiplier = 1.0;
		let mut scroll_direction = self.lock_data().scroll(msg.author.id.0).unwrap_or(etterna::ScrollDirection::Upscroll);
		let mut segments = Vec::new();

		let extract_snap = |string: &str, user_intended: &mut bool| {
			const ENDINGS: &[&str] = &["st", "sts", "nd", "nds", "rd", "rds", "th", "ths"];

			let characters_to_truncate = ENDINGS.iter().find(|&ending| string.ends_with(ending))?.len();
			// UNWRAP: we're only removing up to the string length, so we can't go out-of-bounds
			let string_without_ending = string.get(..(string.len() - characters_to_truncate)).unwrap();
			let snap: u32 = string_without_ending.parse().ok()?;
			*user_intended = true;
			pattern_draw::FractionalSnap::from_snap_number(snap)
		};
		let extract_noteskin = |string: &str, _user_intended: &mut bool| {
			// make lowercase and remove all special characters
			let mut normalized_noteskin_name = string.to_ascii_lowercase();
			normalized_noteskin_name.retain(|c| c.is_alphanumeric());

			match normalized_noteskin_name.as_str() {
				"dbz" | "dividebyzero" => Some(&self.noteskin_provider.dbz),
				"wafles" | "wafles3" => Some(&self.noteskin_provider.wafles),
				"default" | "lambda" => Some(&self.noteskin_provider.lambda),
				"delta-note" | "delta" => Some(&self.noteskin_provider.delta_note),
				"sbz" | "subtractbyzero" => Some(&self.noteskin_provider.sbz),
				"mbz" | "multiplybyzero" => Some(&self.noteskin_provider.mbz),
				"eo_baner" | "eobaner" => Some(&self.noteskin_provider.eo_baner),
				"rustmania" => Some(&self.noteskin_provider.rustmania),
				_ => None,
			}
		};
		let extract_vertical_spacing_multiplier = |string: &str, user_intended: &mut bool| {
			if !string.ends_with('x') { return None };
			// UNWRAP: at this point the string must have 'x' at the end so we can safely strip one char
			let vertical_spacing_multiplier: f32 = string.get(..(string.len() - 1)).unwrap().parse().ok()?;
			*user_intended = true;
			if vertical_spacing_multiplier > 0.0 {
				Some(vertical_spacing_multiplier)
			} else {
				None
			}
		};
		let extract_scroll_direction = |string: &str, _user_intended: &mut bool| {
			match string.to_lowercase().as_str() {
				"up" => Some(etterna::ScrollDirection::Upscroll),
				"down" | "reverse" => Some(etterna::ScrollDirection::Downscroll),
				_ => None,
			}
		};
		let extract_keymode = |string: &str, user_intended: &mut bool| {
			if !(string.ends_with('k') || string.ends_with('K')) { return None }

			// UNWRAP: at this point the string must have 'k' at the end so we can safely strip one char
			let keymode: u32 = string.get(..(string.len() - 1)).unwrap().parse().ok()?;
			*user_intended = true;
			if keymode > 0 {
				Some(keymode)
			} else {
				None
			}
		};

		let mut pattern_buffer = String::new();
		for arg in args.split_whitespace() {
			let mut did_user_intend = false;
			if let Some(new_snap) = extract_snap(arg, &mut did_user_intend) {
				if pattern_buffer.len() > 0 {
					segments.push((pattern_draw::parse_pattern(&pattern_buffer)?, snap));
					pattern_buffer.clear();
				}
				snap = new_snap;
				continue;
			}
			if did_user_intend {
				msg.channel_id.say(&ctx.http, format!("\"{}\" is not a valid snap", arg))?;
			}

			let mut did_user_intend = false;
			if let Some(noteskin) = extract_noteskin(arg, &mut did_user_intend) {
				noteskin_override = Some(noteskin);
				continue;
			}
			if did_user_intend {
				msg.channel_id.say(&ctx.http, format!("\"{}\" is not a valid noteskin name", arg))?;
			}
			
			let mut did_user_intend = false;
			if let Some(vertical_spacing_multiplier_override) = extract_vertical_spacing_multiplier(arg, &mut did_user_intend) {
				vertical_spacing_multiplier = vertical_spacing_multiplier_override;
				continue;
			}
			if did_user_intend {
				msg.channel_id.say(&ctx.http, format!("\"{}\" is not a valid zoom option", arg))?;
			}
			
			let mut did_user_intend = false;
			if let Some(scroll_direction_override) = extract_scroll_direction(arg, &mut did_user_intend) {
				scroll_direction = scroll_direction_override;
				continue;
			}
			if did_user_intend {
				msg.channel_id.say(&ctx.http, format!("\"{}\" is not a valid scroll direction", arg))?;
			}
			
			let mut did_user_intend = false;
			if let Some(keymode) = extract_keymode(arg, &mut did_user_intend) {
				keymode_override = Some(keymode);
				continue;
			}
			if did_user_intend {
				msg.channel_id.say(&ctx.http, format!("\"{}\" is not a valid keymode", arg))?;
			}
			
			// if nothing matched, this is just an ordinary part of the pattern
			pattern_buffer += arg;
		}
		if pattern_buffer.len() > 0 {
			segments.push((pattern_draw::parse_pattern(&pattern_buffer)?, snap));
			pattern_buffer.clear();
		}
		
		let keymode = if let Some(keymode) = keymode_override {
			keymode
		} else {
			let highest_lane = segments.iter()
				.flat_map(|(pattern, _)| &pattern.rows)
				// if the user entered `+pattern ldr`, was the highest column 3, or 4? remember, the
				// meaning of `r` depends on keymode, but we don't know the keymode yet. I've
				// decided to assume 4k in the fallback case
				.filter_map(|row| row.iter().map(|(lane, _note_type)| lane.column_number_with_keymode(4)).max())
				.max().ok_or(Error::PatternVisualizeError(pattern_draw::Error::EmptyPattern))?;
			let keymode = (highest_lane + 1) as u32;
			keymode.max(4) // clamp keymode to a minimum of 4k. yes, 3k exists, but it's so niche that even if only three lanes are populated, the pattern is probably meant to be 4k
		};

		let noteskin = if let Some(noteskin) = noteskin_override {
			&noteskin
		} else {
			// choose a default noteskin
			match keymode {
				3 | 4 | 6 | 8 => &self.noteskin_provider.dbz,
				5 | 10 => &self.noteskin_provider.delta_note,
				7 | 9 => &self.noteskin_provider.sbz,
				_ => &self.noteskin_provider.sbz, // fallback
			}
		};

		let generated_pattern = pattern_draw::draw_pattern(pattern_draw::PatternRecipe {
			noteskin,
			scroll_direction,
			keymode: keymode as usize /* I thought I had changedit to u32 in pattern_draw???? */,
			vertical_spacing_multiplier,
			pattern: &segments,
			max_image_dimensions: (5000, 10000),
			max_sprites: 1000,
		})?;

		let mut img_bytes = Vec::with_capacity(1_000_000); // preallocate 1 MB for the img
		image::DynamicImage::ImageRgba8(generated_pattern).write_to(
			&mut img_bytes,
			image::ImageOutputFormat::Png
		).map_err(pattern_draw::Error::ImageError)?;

		// Send the image into the channel where the summoning message comes from
		msg.channel_id.send_files(
			&ctx.http,
			vec![(img_bytes.as_slice(), "output.png")],
			|m| m
		)?;

		Ok(())
	}

	fn profile_compare(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		me: &str,
		you: &str,
		expanded: bool,
	) -> Result<(), Error> {
		let me = self.v2()?.user_details(me)?;
		let you = self.v2()?.user_details(you)?;

		let my_rating = me.rating.calc_player_overall_pre_070();
		let your_rating = you.rating.calc_player_overall_pre_070();

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
			if my_rating < min_ss_rating { min_ss_rating = my_rating; }
			if your_rating < min_ss_rating { min_ss_rating = your_rating; }
			if my_rating > max_ss_rating { max_ss_rating = my_rating; }
			if your_rating > max_ss_rating { max_ss_rating = your_rating; }
		}

		let bar_graph_block = if expanded {
			let mut bar_graph_block = "```prolog\n".to_owned();
			for skillset in etterna::Skillset8::iter() {
				let my_rating = my_rating.get(skillset);
				let your_rating = your_rating.get(skillset);
				bar_graph_block += &format!(
					"{: >10}:   \"░▒▓{}\"\n              “░▒▓{}“\n\n",
					skillset.to_string(), // to_string, or the padding won't work
					gen_unicode_block_bar(18, rescale(my_rating, min_ss_rating..max_ss_rating, 0.0..1.0)),
					gen_unicode_block_bar(18, rescale(your_rating, min_ss_rating..max_ss_rating, 0.0..1.0)),
				)
			}
			bar_graph_block += "```";
			Some(bar_graph_block)
		} else {
			None
		};

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| {
			e
				.color(crate::ETTERNA_COLOR)
				.title(format!(
					"{} {} vs. {} {}",
					country_code_to_flag_emoji(&me.country_code).unwrap_or_else(|| "❓".into()),
					me.username,
					you.username,
					country_code_to_flag_emoji(&you.country_code).unwrap_or_else(|| "❓".into()),
				))
				.description(string);
			
			if let Some(bar_graph_block) = bar_graph_block {
				e.field(
					format!("Above is {}, below is {}", me.username, you.username),
					bar_graph_block,
					false
				);
			}
			
			e
		}))?;

		Ok(())
	}
	
	// usernames slice must contain at least one element!
	fn skillgraph(&self,
		ctx: &serenity::Context,
		channel_id: serenity::ChannelId,
		usernames: &[&str],
	) -> Result<(), Error> {
		assert!(usernames.len() >= 1);

		if usernames.len() > 20 {
			channel_id.say(&ctx.http, "Relax, now. 10 simultaneous skillgraphs ought to be enough")?;
			return Ok(());
		}

		match usernames {
			[username] => channel_id.say(&ctx.http, format!(
				"Requesting data for {} (this may take a while)",
				username,
			))?,
			[usernames @ .., last] => channel_id.say(&ctx.http, format!(
				"Requesting data for {} and {} (this may take a while)",
				usernames.join(", "),
				last,
			))?,
			[] => unreachable!(),
		};

		fn download_skill_timeline<'a>(
			username: &str,
			web_session: &eo::web::Session,
			storage: &'a mut Option<eo::web::UserScores>,
		) -> Result<etterna::SkillTimeline<&'a str>, Error> {
			let user_id = web_session.user_details(&username)?.user_id;
			let scores = web_session.user_scores(
				user_id,
				..,
				None,
				eo::web::UserScoresSortBy::Date,
				eo::web::SortDirection::Ascending,
				false, // exclude invalid
			)?;

			*storage = Some(scores);
			let scores = storage.as_ref().expect("impossible");

			Ok(etterna::skill_timeline(
				scores.scores.iter().filter_map(|score| {
					Some((
						score.date.as_str(),
						score.validity_dependant.as_ref()?.nerfed_ssr().into(),
					))
				}),
				true,
			))
		}

		const MAX_SIMULTANEOUS_DOWNLOADS: usize = 3;

		let mut storages = (0..usernames.len()).map(|_| None).collect::<Vec<_>>();
		let mut skill_timelines = Vec::with_capacity(usernames.len());
		for (username_chunk, storage_chunk) in usernames.chunks(MAX_SIMULTANEOUS_DOWNLOADS).zip(storages.chunks_mut(MAX_SIMULTANEOUS_DOWNLOADS)) {
			let join_handles = username_chunk.iter().zip(storage_chunk)
				.map(|(username, storage)| {
					// SAFETY: this is safe as long as the returned handle is not leaked, which we're not doing
					unsafe {
						thread_scoped::scoped(move || {
							download_skill_timeline(username, &self.web_session, storage)
						})
					}
				})
				.collect::<Vec<_>>();
			
			for join_handle in join_handles {
				skill_timelines.push(join_handle.join()?);
			}
		}

		draw_skill_graph::draw_skill_graph(
			&skill_timelines,
			&usernames,
			"output.png"
		).map_err(Error::SkillGraphError)?;

		channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;

		Ok(())
	}

	fn command(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		args: &str
	) -> Result<(), Error> {
		println!("Executing command '{}' with args '{}'", cmd, args);

		if cmd.starts_with("top") {
			// UNWRAP: we can safely strip because we checked that the string has it at the start
			if let Ok(limit) = cmd.get(3..).unwrap().parse() {
				self.top_scores(ctx, msg, args, limit)?;
			} else {
				msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			}
			return Ok(());
		}

		match cmd {
			"ping" => {
				let mut response = String::from("Pong");
				for _ in 0..args.matches("ping").count() {
					response += " pong";
				}
				response += "!";
				msg.channel_id.say(&ctx.http, &response)?;
			},
			"help" => {
				msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
					.description(self.make_help_message(args.eq_ignore_ascii_case("pattern")))
					.color(crate::ETTERNA_COLOR)
				))?;
			},
			"profile" => {
				self.profile(ctx, msg, args)?;
			},
			"advprof" => {
				msg.channel_id.say(&ctx.http, "Note: +profile now does the same thing as +advprof; there's no reason to use +advprof anymore")?;
				self.profile(ctx, msg, args)?;
			}
			"lastsession" | "ls" => {
				self.latest_scores(ctx, msg, args)?;
			},
			"pattern" => {
				self.pattern(ctx, msg, args)?;
			},
			"servers" => {
				let guilds = ctx.http.get_current_user()?.guilds(&ctx.http)?;

				let mut response = format!("I am currently in {} servers!\n", guilds.len());
				for guild in guilds {
					response += &format!("- {}\n", guild.name);
				}

				msg.channel_id.say(&ctx.http, response)?;
			},
			"uptime" => {
				let uptime = std::time::Instant::now() - self.start_time;
				
				let div_mod = |a, b| (a / b, a % b);
				
				let millis = uptime.as_millis();
				let (seconds, millis) = div_mod(millis, 1000);
				let (minutes, seconds) = div_mod(seconds, 60);
				let (hours, minutes) = div_mod(minutes, 60);
				let (days, hours) = div_mod(hours, 24);

				msg.channel_id.say(&ctx.http, format!(
					"Duration since last restart: {}:{:02}:{:02}:{:02}.{:03}",
					days, hours, minutes, seconds, millis
				))?;
			},
			"skillgraph" => {
				let usernames = args.split_whitespace().collect::<Vec<_>>();
				if usernames.len() == 0 {
					self.skillgraph(ctx, msg.channel_id, &[&self.get_eo_username(ctx, msg)?])?;
				} else {
					self.skillgraph(ctx, msg.channel_id, &usernames)?;
				}
			},
			"random" | "randomscore" => {
				let username = match args.split_ascii_whitespace().next() {
					Some(x) => x.to_owned(),
					None => self.get_eo_username(ctx, msg)?,
				};

				let mut data = self.lock_data();
				let user = data.user_registry.iter_mut()
					.find(|user| user.eo_username.eq_ignore_ascii_case(&username))
					.ok_or(Error::UserNotInRegistry)?;
				
				let user_eo_id = user.eo_id;
				
				// find a random score. If it's invalid, find another one
				let scorekey = loop {
					let score = get_random_score(user, &self.web_session)?;
					if let Some(validity_dependant) = score.validity_dependant {
						break validity_dependant.scorekey;
					}
				};
				drop(data);
				
				self.score_card(ctx, msg.channel_id, ScoreCard {
					scorekey: &scorekey,
					triggerers: None,
					user_id: Some(user_eo_id),
					show_ssrs_and_judgements_and_modifiers: true,
					alternative_judge: extract_judge_from_string(args),
				})?;
			}
			"lookup" => {
				if args.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_LOOKUP_HELP)?;
					return Ok(());
				}

				let data = self.lock_data();
				let user = data.user_registry.iter()
					.find(|user| user.discord_username.eq_ignore_ascii_case(args))
					.ok_or(Error::UserNotInRegistry)?;
				
				msg.channel_id.say(&ctx.http, format!(
					"Discord username: {}\nEO username: {}\nhttps://etternaonline.com/user/{}",
					user.discord_username,
					user.eo_username,
					user.eo_username,
				))?;
			},
			"quote" => {
				let quote_index = rand::thread_rng().gen_range(0, self.config.quotes.len());
				// UNWRAP: index is below quotes len because we instructed the rand crate to do so
				let quote = self.config.quotes.get(quote_index).unwrap();
				let string = match &quote.source {
					Some(source) => format!("> {}\n~ {}", quote.quote, source),
					None => format!("> {}", quote.quote),
				};
				msg.channel_id.say(&ctx.http, &string)?;
			},
			// "scroll" => {
			// 	msg.channel_id.say(
			// 		&ctx.http,
			// 		"Go to song options (hit enter twice when starting a song)\nScroll -> Reverse"
			// 	)?;
			// }
			// "scrolll" => {
			// 	const SCROLLL_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(60);

			// 	let now = std::time::Instant::now();
			// 	if self.last_scrolll + SCROLLL_COOLDOWN > now {
			// 		msg.channel_id.say(&ctx.http, "Cool down a bit with that");
			// 		std::thread::sleep(now - self.last_scrolll);
			// 		self.last_scrolll = now;
			// 	}

			// 	msg.channel_id.send_files(&ctx.http, vec![
			// 		"assets/ETTERNATUTORIAL00.png",
			// 		"assets/ETTERNATUTORIAL01.png",
			// 		"assets/ETTERNATUTORIAL02.png",
			// 		"assets/ETTERNATUTORIAL03.png",
			// 		"assets/ETTERNATUTORIAL04.png",
			// 	], |m| m)?;
			// }
			"rs" => {
				let args: Vec<_> = args.split_whitespace().collect();
				let (eo_username, alternative_judge) = match *args.as_slice() {
					[] => (self.get_eo_username(ctx, msg)?, None),
					[username_or_judge_string] => {
						if let Some(judge) = extract_judge_from_string(username_or_judge_string) {
							(self.get_eo_username(ctx, msg)?, Some(judge))
						} else {
							(username_or_judge_string.to_owned(), None)
						}
					}
					[username, judge_string] => {
						if let Some(judge) = extract_judge_from_string(judge_string) {
							(username.to_owned(), Some(judge))
						} else {
							msg.channel_id.say(&ctx.http, CMD_RS_HELP)?;
							return Ok(());
						}
					},
					_ => {
						msg.channel_id.say(&ctx.http, CMD_RS_HELP)?;
						return Ok(());
					}
				};

				let latest_scores = self.v2()?.user_latest_scores(&eo_username)?;
				let latest_score = match latest_scores.first() {
					Some(x) => x,
					None => {
						msg.channel_id.say(&ctx.http, "User has no scores")?;
						return Ok(());
					}
				};

				let user_id = self.get_eo_user_id(&eo_username)?;
				self.score_card(ctx, msg.channel_id, ScoreCard {
					scorekey: &latest_score.scorekey,
					user_id: Some(user_id),
					show_ssrs_and_judgements_and_modifiers: true,
					alternative_judge,
					triggerers: None,
				})?;
			}
			"scrollset" => {
				let scroll = match &args.to_lowercase() as &str {
					"down" | "downscroll" => etterna::ScrollDirection::Downscroll,
					"up" | "upscroll" => etterna::ScrollDirection::Upscroll,
					"" => {
						msg.channel_id.say(&ctx.http, CMD_SCROLLSET_HELP)?;
						return Ok(());
					},
					_ => {
						msg.channel_id.say(&ctx.http, format!("No such scroll '{}'", args))?;
						return Ok(());
					},
				};
				self.lock_data().set_scroll(msg.author.id.0, scroll);
				msg.channel_id.say(&ctx.http, &format!("Your scroll type is now {:?}", scroll))?;
			}
			"userset" => {
				if args.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_USERSET_HELP)?;
					return Ok(());
				}
				
				let new_user_entry = config::UserRegistryEntry {
					discord_id: msg.author.id.0,
					discord_username: msg.author.name.to_owned(),
					eo_id: self.web_session.user_details(args)?.user_id,
					eo_username: args.to_owned(),
					last_known_num_scores: None,
				};
				
				let mut data = self.lock_data();
				match data.user_registry.iter_mut().find(|u| u.discord_id == msg.author.id.0) {
					Some(existing_user_entry) => {
						msg.channel_id.say(&ctx.http, format!(
							"Successfully updated username from `{}` to `{}`",
							existing_user_entry.eo_username,
							new_user_entry.eo_username,
						))?;

						*existing_user_entry = new_user_entry;
					},
					None => {
						msg.channel_id.say(&ctx.http, format!(
							"Successfully set username to `{}`",
							args
						))?;

						data.user_registry.push(new_user_entry);
					},
				};
			},
			"rivalset" => {
				if args.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_RIVALSET_HELP)?;
					return Ok(());
				}
				if let Err(eo::Error::UserNotFound) = self.v2()?.user_details(args) {
					msg.channel_id.say(&ctx.http, &format!("User `{}` doesn't exist", args))?;
					return Ok(());
				}

				let response = match self.lock_data().set_rival(
					msg.author.id.0,
					args.to_owned()
				) {
					Some(old_rival) => format!(
						"Successfully updated your rival from `{}` to `{}`",
						old_rival,
						args,
					),
					None => format!("Successfully set your rival to `{}`", args),
				};
				msg.channel_id.say(&ctx.http, &response)?;
			},
			"rival" => {
				let me = &self.get_eo_username(ctx, msg)?;
				let you = match self.lock_data().rival(msg.author.id.0) {
					Some(rival) => rival.to_owned(),
					None => {
						msg.channel_id.say(&ctx.http, "Set your rival first with `+rivalset USERNAME`")?;
						return Ok(());
					}
				};
				
				let expanded = args == "expanded";

				self.profile_compare(ctx, msg, me, &you, expanded)?;
			},
			"rivalgraph" => {
				let me = self.get_eo_username(ctx, msg)?;
				let you = match self.lock_data().rival(msg.author.id.0) {
					Some(rival) => rival.to_owned(),
					None => {
						msg.channel_id.say(&ctx.http, "Set your rival first with `+rivalset USERNAME`")?;
						return Ok(());
					}
				};
				self.skillgraph(ctx, msg.channel_id, &[&me, &you])?;
			}
			"compare" => {
				let args: Vec<&str> = args.split_whitespace().collect();

				let (me, you, expanded) = match *args.as_slice() {
					[you] => (self.get_eo_username(ctx, msg)?, you, false),
					[you, "expanded"] => (self.get_eo_username(ctx, msg)?, you, true),
					[me, you] => (me.to_owned(), you, false),
					[me, you, "expanded"] => (me.to_owned(), you, true),
					_ => {
						msg.channel_id.say(&ctx.http, CMD_COMPARE_HELP)?;
						return Ok(());
					}
				};

				self.profile_compare(ctx, msg, &me, you, expanded)?;
			}
			_ => {},
		}
		Ok(())
	}

	fn song_card(&self,
		_ctx: &serenity::Context,
		_msg: &serenity::Message,
		song_id: u32,
	) -> Result<(), Error> {
		println!("Argh I really _want_ to show song info for {}, but the EO v2 API doesn't expose \
			the required functions :(", song_id);
		Ok(())
	}

	fn score_card(&self,
		ctx: &serenity::Context,
		channel_id: serenity::ChannelId,
		info: ScoreCard<'_>,
	) -> Result<(), Error> {
		let score = self.v2()?.score_data(info.scorekey)?;

		let alternative_judge_wifescore = if let Some(alternative_judge) = info.alternative_judge {
			if let Some(replay) = &score.replay {
				etterna::rescore_from_note_hits::<etterna::Wife3, _>(
					replay.notes.iter().map(|note| note.hit),
					score.judgements.hit_mines,
					score.judgements.let_go_holds + score.judgements.missed_holds,
					alternative_judge,
				)
			} else {
				None
			}
		} else {
			None
		};

		let mut description = String::new();
		if let Some((triggerers, trigger_msg)) = info.triggerers {
			let (server_id, channel_id, msg_id) = trigger_msg;
			description += &format!(
				"_[Requested](https://discord.com/channels/{}/{}/{}) by ",
				server_id,
				channel_id,
				msg_id
			);
			for user in triggerers.iter() {
				description += &format!("<@{}>, ", user.id);
			}
			description.truncate(description.len() - 2); // Remove trailing ", "
			description += "_\n";
		}
		if let Some(user_id) = info.user_id {
			description += &format!("https://etternaonline.com/score/view/{}{}\n", info.scorekey, user_id);
		}
		if info.show_ssrs_and_judgements_and_modifiers {
			description += &format!("```\n{}\n```", score.modifiers);
		}
		description += &format!(r#"```nim
{}
   Max Combo: {:<5}   ⏐        Perfect: {}
     Overall: {:<5.2}   ⏐          Great: {}
      Stream: {:<5.2}   ⏐           Good: {}
     Stamina: {:<5.2}   ⏐            Bad: {}
  Jumpstream: {:<5.2}   ⏐           Miss: {}
  Handstream: {:<5.2}   ⏐      Hit Mines: {}
       Jacks: {:<5.2}   ⏐     Held Holds: {}
   Chordjack: {:<5.2}   ⏐  Dropped Holds: {}
   Technical: {:<5.2}   ⏐   Missed Holds: {}
```
"#,
			if let Some(alternative_judge_wifescore) = alternative_judge_wifescore {
				format!(
					concat!(
						"        Wife: {:<5.2}%  ⏐\n",
						"     Wife {}: {:<5.2}%  ⏐      Marvelous: {}",
					),
					score.wifescore.as_percent(),
					// UWNRAP: if alternative_judge_wifescore is Some, info.alternative_judge is too
					info.alternative_judge.unwrap().name,
					alternative_judge_wifescore.as_percent(),
					score.judgements.marvelouses,
				)
			} else {
				format!(
					"        Wife: {:<5.2}%  ⏐      Marvelous: {}",
					score.wifescore.as_percent(), score.judgements.marvelouses,
				)
			},
			score.max_combo, score.judgements.perfects,
			score.ssr.overall, score.judgements.greats,
			score.ssr.stream, score.judgements.goods,
			score.ssr.stamina, score.judgements.bads,
			score.ssr.jumpstream, score.judgements.misses,
			score.ssr.handstream, score.judgements.hit_mines,
			score.ssr.jackspeed, score.judgements.held_holds,
			score.ssr.chordjack, score.judgements.let_go_holds,
			score.ssr.technical, score.judgements.missed_holds,
		);

		struct ScoringSystemComparison {
			wife2_score: etterna::Wifescore,
			wife3_score: etterna::Wifescore,
			wife3_score_zero_mean: etterna::Wifescore,
		}

		struct ReplayAnalysis {
			replay_graph_path: &'static str,
			scoring_system_comparison_j4: ScoringSystemComparison,
			scoring_system_comparison_alternative: Option<ScoringSystemComparison>,
			fastest_finger_jackspeed: f32, // NPS, single finger
			fastest_nps: f32,
			longest_100_combo: u32,
			longest_marv_combo: u32,
			longest_perf_combo: u32,
			longest_combo: u32,
			mean_offset: f32,
		}


		let do_replay_analysis = |score: &eo::v2::ScoreData| -> Option<Result<ReplayAnalysis, Error>> {
			use etterna::SimpleReplay;

			let replay = score.replay.as_ref()?;

			let r = replay_graph::generate_replay_graph(replay, "replay_graph.png").transpose()?;
			if let Err(e) = r {
				return Some(Err(Error::ReplayGraphError(e)))
			}
			
			// in the following, DONT scale find_fastest_note_subset results by rate - I only needed
			// to do that for etterna-graph where the note seconds where unscaled. EO's note seconds
			// _are_ scaled though.

			let lanes = replay.split_into_lanes()?;
			let mut max_finger_nps = 0.0;
			for lane in &lanes {
				let this_fingers_max_nps = etterna::find_fastest_note_subset(&lane.hit_seconds, 20, 20).speed;

				if this_fingers_max_nps > max_finger_nps {
					max_finger_nps = this_fingers_max_nps;
				}
			}

			let note_and_hit_seconds = replay.split_into_notes_and_hits()?;
			let unsorted_hit_seconds = note_and_hit_seconds.hit_seconds;

			let mut sorted_hit_seconds = unsorted_hit_seconds;
			// UNWRAP: if one of those values is NaN... something is pretty wrong
			sorted_hit_seconds.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
			let sorted_hit_seconds = sorted_hit_seconds;

			let fastest_nps = etterna::find_fastest_note_subset(&sorted_hit_seconds, 100, 100).speed;

			let mean_offset = replay.mean_deviation();
			let replay_zero_mean = eo::Replay {
				notes: replay.notes.iter()
					.map(|note| {
						let mut note = note.clone();
						if let etterna::Hit::Hit { deviation } = &mut note.hit {
							*deviation -= mean_offset;
						}
						note
					})
					.collect(),
			};
			
			Some(Ok(ReplayAnalysis {
				replay_graph_path: "replay_graph.png",
				scoring_system_comparison_j4: ScoringSystemComparison {
					wife2_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife2>(
						replay,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
					wife3_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife3>(
						replay,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
					wife3_score_zero_mean: eo::rescore::<etterna::NaiveScorer, etterna::Wife3>(
						&replay_zero_mean,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
				},
				scoring_system_comparison_alternative: match info.alternative_judge {
					Some(alternative_judge) => Some(ScoringSystemComparison {
						wife2_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife2>(
							replay,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
						wife3_score: eo::rescore::<etterna::NaiveScorer, etterna::Wife3>(
							replay,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
						wife3_score_zero_mean: eo::rescore::<etterna::NaiveScorer, etterna::Wife3>(
							&replay_zero_mean,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
					}),
					None => None,
				},
				fastest_finger_jackspeed: max_finger_nps,
				fastest_nps,
				longest_100_combo: replay.longest_combo(|hit| hit.is_within_window(0.005)),
				longest_marv_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.marvelous_window)),
				longest_perf_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.perfect_window)),
				longest_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.great_window)),
				mean_offset,
			}))
		};

		let replay_analysis = do_replay_analysis(&score).transpose()?;

		channel_id.send_message(&ctx.http, |m| {
			m.embed(|e| {
				e
					.color(crate::ETTERNA_COLOR)
					.author(|a| a
						.name(&score.song_name)
						.url(format!("https://etternaonline.com/song/view/{}", score.song_id))
						.icon_url(format!("https://etternaonline.com/img/flags/{}.png", score.user.country_code))
					)
					// .thumbnail(format!("https://etternaonline.com/avatars/{}", score.user.avatar)) // takes too much space
					.description(description)
					.footer(|f| f
						.text(format!("Played by {}", &score.user.username))
						.icon_url(format!("https://etternaonline.com/avatars/{}", score.user.avatar))
					);
				
				if let Some(analysis) = &replay_analysis {
					let alternative_text_1;
					let alternative_text_2;
					let alternative_text_4;
					if let Some(comparison) = &analysis.scoring_system_comparison_alternative {
						// UNWRAP: if we're in this branch, info.alternative_judge is Some
						alternative_text_1 = format!(", {:.2} on {}", comparison.wife2_score, info.alternative_judge.unwrap().name);
						alternative_text_2 = format!(", {:.2} on {}", comparison.wife3_score, info.alternative_judge.unwrap().name);
						alternative_text_4 = format!(", {:.2} on {}", comparison.wife3_score_zero_mean, info.alternative_judge.unwrap().name);
					} else {
						alternative_text_1 = "".to_owned();
						alternative_text_2 = "".to_owned();
						alternative_text_4 = "".to_owned();
					}

					e
						.attachment(analysis.replay_graph_path)
						.field("Score comparisons", format!(
							concat!(
								"{}",
								"**Wife2**: {:.2}%{}\n",
								"**Wife3**: {:.2}%{}\n",
								"**Wife3**: {:.2}%{} (mean of {:.1}ms corrected)",
							),
							if (analysis.scoring_system_comparison_j4.wife3_score.as_percent() - score.wifescore.as_percent()).abs() > 0.01 {
								"_Note: these calculated scores are slightly inaccurate_\n"
							} else {
								""
							},
							analysis.scoring_system_comparison_j4.wife2_score.as_percent(),
							alternative_text_1,
							analysis.scoring_system_comparison_j4.wife3_score.as_percent(),
							alternative_text_2,
							analysis.scoring_system_comparison_j4.wife3_score_zero_mean.as_percent(),
							alternative_text_4,
							analysis.mean_offset * 1000.0,
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

	pub fn message(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		was_explicitly_invoked: &mut bool,
	) -> Result<(), Error> {
		// Let's not do this, because if a non existing command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		// If the message is in etternaonline server, and not in an allowed channel, and not sent
		// by a person with the permission to manage the guild, don't process the command
		let user_is_allowed_bot_interaction = {
			if let Some(guild_member) = get_guild_member(ctx, msg)? { // if msg is in server (opposed to DMs)
				if guild_member.guild_id.0 == self.config.etterna_online_guild_id
					&& !self.config.allowed_channels.contains(&msg.channel_id.0)
					&& !guild_member.permissions(&ctx.cache)?.manage_guild()
				{
					false
				} else {
					true
				}
			} else {
				true
			}
		};

		self.check_potential_score_screenshot(ctx, msg)?;

		if msg.channel_id.0 == self.config.work_in_progress_channel {
			let num_links = LINK_REGEX.find_iter(&msg.content).count();
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
			let alternative_judge = extract_judge_from_string(&msg.content);
			for groups in SCORE_LINK_REGEX.captures_iter(&msg.content) {
				// UNWRAP: regex has this group
				let scorekey = match etterna::Scorekey::new(groups.get(1).unwrap().as_str().to_owned()) {
					Some(valid_scorekey) => valid_scorekey,
					None => continue,
				};

				// UNWRAP: regex has this group
				let user_id_group = groups.get(2).unwrap().as_str();
				let user_id: u32 = match user_id_group.parse() {
					Ok(x) => x,
					Err(e) => {
						// UNWRAP: regex has this group
						println!("Error while parsing '{}' (\\d+) as u32: {}", user_id_group, e);
						continue;
					}
				};
				
				println!("Trying to show score card for scorekey {} user id {}", scorekey, user_id);
				if let Err(e) = self.score_card(&ctx, msg.channel_id, ScoreCard {
					scorekey: &scorekey,
					user_id: None,
					show_ssrs_and_judgements_and_modifiers: true,
					alternative_judge,
					triggerers: None,
				}) {
					println!("Error while showing score card for {}: {}", scorekey, e);
				}
			}
	
			for groups in SONG_LINK_REGEX.captures_iter(&msg.content) {
				println!("{:?}", groups);
				// UNWRAP: regex has this group
				let song_id = match groups.get(1).unwrap().as_str().parse() {
					Ok(song_id) => song_id,
					Err(_) => continue, // this wasn't a valid song view url after all
				};

				println!("Trying to show score card for song id {}", song_id);
				if let Err(e) = self.song_card(&ctx, &msg, song_id) {
					println!("Error while showing song card for {}: {}", song_id, e);
				}
			}
		}

		if msg.content.starts_with('+') {
			*was_explicitly_invoked = true;

			// UNWRAP: we just checked it has a string at the beginning that we can chop away
			let text = &msg.content.get(1..).unwrap();

			// Split message into command part and parameter part
			let mut a = text.splitn(2, ' ');
			// UNWRAP: msg.content can't be empty, hence the token iterator has at least one elem
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

	pub fn check_member_update_for_max_300(&self,
		ctx: serenity::Context,
		old: serenity::Member,
		new: serenity::Member
	) -> Result<(), Error> {
		let guild = new.guild_id.to_partial_guild(&ctx.http)?;
		
		let get_guild_role = |guild_id| {
			if let Some(guild) = guild.roles.get(guild_id) {
				Some(guild.name.as_str())
			} else {
				println!("Couldn't find role {:?} in guild roles ({:?})... weird", guild_id, guild.roles);
				None
			}
		};

		let has_max_300_now = new.roles.iter().any(|r| get_guild_role(r) == Some("MAX 300"));
		let had_max_300_previously = old.roles.iter().any(|r| get_guild_role(r) == Some("MAX 300"));
		
		if has_max_300_now && !had_max_300_previously {
			ctx.http.get_channel(self.config.promotion_gratulations_channel)?
				// UNWRAP: we verified in Self::load()
				.guild().unwrap().read()
				.say(
					&ctx.http,
					format!("Congrats on the promotion, <@{}>!", old.user_id()
				)
			)?;
		}

		Ok(())
	}

	pub fn guild_member_update(&self,
		ctx: serenity::Context,
		old: Option<serenity::Member>,
		new: serenity::Member
	) -> Result<(), Error> {
		if let Some(user_entry) = self.lock_data().user_registry.iter_mut()
			.find(|user| user.discord_id == new.user.read().id.0)
		{
			user_entry.discord_username = new.user.read().name.clone();
		}

		if let Some(old) = old {
			self.check_member_update_for_max_300(ctx, old, new)?;
		}

		Ok(())
	}

	pub fn check_potential_score_screenshot(&self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<(), Error> {
		let guild_id = match msg.guild_id {
			Some(x) => x,
			None => return Ok(()), // this msg was sent in DMs
		};

		if msg.channel_id.0 != self.config.score_channel {
			return Ok(());
		}

		let attachment = match msg.attachments.iter().find(|a| a.width.is_some()) {
			Some(a) => a,
			None => return Ok(()), // non-image post in score channel. Ignore
		};

		// sigh, I wish serenity had nice things, like methods built-in for this
		let member = get_guild_member(&ctx, &msg)?;

		if let Some(member) = member { // if was sent in a guild (as opposed to DMs)
			// If message was sent in EO and user doesn't have the appropriate role for the
			// score OCR feature, ignore this image
			if member.guild_id.0 == self.config.etterna_online_guild_id {
				let has_required_role = member.roles.iter().any(|r| r.0 == self.config.score_ocr_allowed_eo_role);
				if !has_required_role {
					return Ok(());
				}
			}
		}

		let bytes = attachment.download()?;
		println!("Post from {} on {:?}...", &msg.author.name, &msg.timestamp);
		let recognized = score_ocr::EvaluationScreenData::recognize_from_image_bytes(&bytes)?;
		println!("Recognized {:?}", recognized);

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
			let validity_dependant = match score.validity_dependant {
				Some(x) => x,
				None => continue, // don't check invalid scores (we don't have scorekey for those)
			};

			let score_as_eval = score_ocr::EvaluationScreenData {
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
			for (theme_i, recognized) in recognized.iter().enumerate() { // check results for all themes
				let equality_score = recognized.equality_score(&score_as_eval);
				if equality_score > best_equality_score {
					best_equality_score = equality_score;
					best_theme_i = theme_i;
				}
			}
			let equality_score = best_equality_score;
			let _theme_i = best_theme_i;
			// println!("Found match in theme {}", theme_i);

			if equality_score > score_ocr::MINIMUM_EQUALITY_SCORE_TO_BE_PROBABLY_EQUAL
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
		self.ocr_score_card_manager.lock().add_candidate(guild_id, msg.channel_id, msg.id, msg.author.id, scorekey, user_id);

		Ok(())
	}

	pub fn reaction_add(&self,
		ctx: serenity::Context,
		reaction: serenity::Reaction,
	) -> Result<(), Error> {
		if reaction.user_id == self.user_id {
			return Ok(());
		}

		if let Some(score_info) = self.ocr_score_card_manager.lock().add_reaction(&ctx, &reaction)? {
			// borrow checker headaches because this thing is monolithic
			let reactors: Vec<serenity::User> = score_info.reactors.iter().cloned().collect();
			let scorekey = score_info.scorekey.clone();
			let eo_user_id = score_info.eo_user_id;
			let trigger_msg = score_info.trigger_msg;

			self.score_card(&ctx, self.config.score_ocr_card_channel.into(), ScoreCard {
				scorekey: &scorekey,
				user_id: Some(eo_user_id),
				show_ssrs_and_judgements_and_modifiers: false,
				alternative_judge: None,
				triggerers: Some((&reactors, trigger_msg)),
			})?;
		}

		Ok(())
	}
}

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

struct ScoreCardTrigger<'a> {
	scorekey: &'a etterna::Scorekey,
	eo_user_id: u32,
	reactors: &'a std::collections::HashSet<serenity::User>,
	trigger_msg: (serenity::GuildId, serenity::ChannelId, serenity::MessageId),
}

struct OcrScoreCardManager {
	candidates: Vec<Candidate>,
}

impl OcrScoreCardManager {
	pub fn new() -> Self {
		Self { candidates: vec![] }
	}

	pub fn add_candidate(&mut self,
		guild_id: serenity::GuildId,
		channel_id: serenity::ChannelId,
		message_id: serenity::MessageId,
		author_id: serenity::UserId,
		scorekey: etterna::Scorekey,
		user_id: u32,
	) {
		println!("Added new candidate {}, author id {}", &scorekey, author_id.0);
		self.candidates.push(Candidate {
			guild_id, channel_id, message_id, author_id, scorekey, user_id,
			
			reactors: std::collections::HashSet::new(),
			score_card_has_been_printed: false,
		});
	}

	/// Returns the score scorekey and user id if this reaction triggers the score card
	pub fn add_reaction(&mut self,
		ctx: &serenity::Context,
		reaction: &serenity::Reaction,
	) -> Result<Option<ScoreCardTrigger>, Error> {
		println!("Got reaction in score ocr card manager");

		// Let's check that the user even clicked the correct emoji type
		if reaction.emoji != serenity::ReactionType::Unicode("🔍".to_owned()) {
			return Ok(None);
		}

		// Find the Candidate that this reaction was made on, or return if the user made the
		// reaction on some unrelated message, i.e. a non-candidate
		let mut candidate = match self.candidates.iter_mut()
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
				reactors: &candidate.reactors,
				trigger_msg: (candidate.guild_id, candidate.channel_id, candidate.message_id),
			})
		} else {
			None
		})
	}
}