mod pattern_visualize;

use crate::serenity; // use my custom serenity prelude
use etternaonline_api as eo;

const BOT_PREFIX: &str = "+";

const CMD_TOP_HELP: &str = "Call this command with `+topNN [USERNAME] [SKILLSET]` (both params optional)";
const CMD_COMPARE_HELP: &str = "Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`";

fn country_code_to_flag_emoji(country_code: &str) -> String {
	let regional_indicator_value_offset = '🇦' as u32 - 'a' as u32;
	country_code
		.to_lowercase()
		.chars()
		.map(|c| std::char::from_u32(c as u32 + regional_indicator_value_offset).unwrap_or(c))
		.collect()
}

struct Config {

}

impl Config {
	fn load() -> Self {
		Self { }
	}

	// we need String here because the string can come either from `self` or from the passed
	// parameter. So we have differing lifetimes which we can't encode with a `&str`
	fn eo_username(&self, discord_username: &str) -> String {
		discord_username.to_owned() // STUB
	}
}

pub struct State {
	config: Config,
	session: eo::Session,
}

impl State {
	pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
		Ok(State {
			session: eo::Session::new_from_login(
				crate::auth::EO_USERNAME.to_owned(),
				crate::auth::EO_PASSWORD.to_owned(),
				crate::auth::EO_CLIENT_DATA.to_owned(),
				std::time::Duration::from_millis(1000),
				Some(std::time::Duration::from_millis(30000)),
			)?,
			config: Config::load(),
		})
	}

	fn top_scores(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
		mut limit: u32,
	) -> Result<(), Box<dyn std::error::Error>> {
		let args: Vec<&str> = text.split_whitespace().collect();

		let skillset;
		let eo_username;
		if args.len() == 0 {
			skillset = None;
			eo_username = self.config.eo_username(&msg.author.name);
		} else if args.len() == 1 {
			match eo::Skillset::from_user_input(args[0]) {
				Some(parsed_skillset) => {
					skillset = Some(parsed_skillset);
					eo_username = self.config.eo_username(&msg.author.name);
				},
				None => {
					skillset = None;
					eo_username = args[0].to_owned(); // to_owned not strictly needed
				},
			}
		} else if args.len() == 2 {
			skillset = match eo::Skillset::from_user_input(args[0]) {
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
			None => self.session.user_top_10_scores(&eo_username),
			Some(skillset) => self.session.user_top_skillset_scores(&eo_username, skillset, limit),
		};
		if let Err(eo::Error::UserNotFound) = top_scores {
			msg.channel_id.say(&ctx.http, format!("No such user \"{}\"", eo_username))?;
			return Ok(());
		}
		let top_scores = top_scores?;

		let country_code = self.session.user_details(&eo_username)?.country_code;

		let mut response = String::from("```");
		for (i, entry) in top_scores.iter().enumerate() {
			response += &format!(
				"{}. {}: {:.2}x\n  ▸ Score: {:.2} Wife: {:.2}%\n",
				i + 1,
				&entry.song_name,
				entry.rate,
				entry.ssr_overall,
				entry.wifescore * 100.0,
			);
		}

		if limit != 10 && skillset == None {
			limit = 10;
			response += "(due to a bug in the EO v2 API, only 10 entries can be shown)";
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

	fn profile_compare(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let args: Vec<&str> = text.split_whitespace().collect();

		let me;
		let you;
		if args.len() == 1 {
			me = self.config.eo_username(&msg.author.name);
			you = args[0];
		} else if args.len() == 2 {
			me = args[0].to_owned();
			you = args[1];
		} else {
			msg.channel_id.say(&ctx.http, CMD_COMPARE_HELP)?;
			return Ok(());
		}

		let me = self.session.user_details(&me)?;
		let you = self.session.user_details(you)?;

		let string = format!(
			r#"
```Prolog
   Overall:   {:.2}  <  {:.2}   {:.2}
    Stream:   {:.2}  <  {:.2}   {:.2}
Jumpstream:   {:.2}  <  {:.2}   {:.2}
Handstream:   {:.2}  <  {:.2}   {:.2}
   Stamina:   {:.2}  <  {:.2}   {:.2}
     Jacks:   {:.2}  <  {:.2}   {:.2}
 Chordjack:   {:.2}  <  {:.2}   {:.2}
 Technical:   {:.2}  <  {:.2}   {:.2}
```			
			"#,
			me.rating.overall(), you.rating.overall(), me.rating.overall() - you.rating.overall(),
			me.rating.stream, you.rating.stream, me.rating.stream - you.rating.stream,
			me.rating.jumpstream, you.rating.jumpstream, me.rating.jumpstream - you.rating.jumpstream,
			me.rating.handstream, you.rating.handstream, me.rating.handstream - you.rating.handstream,
			me.rating.stamina, you.rating.stamina, me.rating.stamina - you.rating.stamina,
			me.rating.jackspeed, you.rating.jackspeed, me.rating.jackspeed - you.rating.jackspeed,
			me.rating.chordjack, you.rating.chordjack, me.rating.chordjack - you.rating.chordjack,
			me.rating.technical, you.rating.technical, me.rating.technical - you.rating.technical,
		);

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

	fn command(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		text: &str
	) -> Result<(), Box<dyn std::error::Error>> {
		if cmd.starts_with("top") {
			if let Ok(limit @ 1..=100) = cmd[3..].parse() {
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
			"user" => {
				let eo_username = if text.is_empty() {
					Some(self.config.eo_username(&msg.author.name))
				} else {
					None
				};
				let eo_username = eo_username.as_deref().unwrap_or(text);

				let reply = match self.session.user_details(&eo_username) {
					Ok(user) => format!("{} {}", user.username, user.player_rating),
					Err(eo::Error::UserNotFound) => format!("User '{}' was not found", eo_username), // TODO: add "maybe you need to add your EO username" msg here
					Err(other) => format!("An error occurred ({})", other),
				};
				msg.channel_id.say(&ctx.http, &reply)?;
			},
			"pattern" => {
				let scroll_type = if text.to_lowercase().starts_with("up") {
					pattern_visualize::ScrollType::Upscroll
				} else if text.starts_with("down") {
					pattern_visualize::ScrollType::Downscroll
				} else {
					pattern_visualize::ScrollType::Upscroll
				};
				pattern_visualize::generate("output.png", text, scroll_type)?;

				// Send the image into the channel where the summoning message comes from
				msg.channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;
			},
			"compare" => {
				self.profile_compare(ctx, msg, text)?;
			}
			_ => {},
		}
		Ok(())
	}

	fn score_card(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		scorekey: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let score = self.session.score_data(scorekey)?;

		let ssrs_string = format!(r#"
```Prolog
      Wife: {:.2}%
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
			score.wifescore * 100.0,
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
```Prolog
Marvelous: {}
  Perfect: {}
    Great: {}
     Good: {}
      Bad: {}
     Miss: {}
```
			"#,
			score.judgements.marvelouses,
			score.judgements.perfects,
			score.judgements.greats,
			score.judgements.goods,
			score.judgements.bads,
			score.judgements.misses,
		);
		let judgements_string = judgements_string.trim();

		msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| e
			.color(crate::ETTERNA_COLOR)
			.thumbnail(format!("https://etternaonline.com/avatars/{}", score.user.avatar))
			.author(|a| a
				.name(&score.song_name)
				.url(format!("https://etternaonline.com/song/view/{}", score.song_id))
				.icon_url(format!("https://etternaonline.com/img/gif/{}.gif", score.user.country_code))
			)
			.description(format!("```\n{}\n```", score.modifiers))
			.field("SSRs", ssrs_string, true)
			.field("Judgements", judgements_string, true)
			.footer(|f| f
				.text(format!("Played by {}", &score.user.username))
			)
		))?;
		Ok(())
	}

	pub fn message(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<(), Box<dyn std::error::Error>> {
		// Let's not do this, because if a non existing command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		for captures in regex::Regex::new(r"https://etternaonline.com/score/view/(S\w{40})(\d+)")
			.unwrap()
			.captures_iter(&msg.content)
		{
			let scorekey = &captures[1];
			let _user_id = &captures[2];
			if let Err(e) = self.score_card(&ctx, &msg, scorekey) {
				println!("Error while showing score card for {}: {}", scorekey, e);
			}
		}

		if msg.content.starts_with(BOT_PREFIX) {
			let text = &msg.content[BOT_PREFIX.len()..];

			// Split message into command part and parameter part
			let mut a = text.splitn(2, ' ');
			let command_name = a.next().unwrap().trim();
			let parameters = a.next().unwrap_or("").trim();
	
			self.command(&ctx, &msg, command_name, parameters)?;
		}

		Ok(())
	}
}