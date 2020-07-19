mod config;
mod pattern_visualize;

use crate::serenity; // use my custom serenity prelude
mod eo {
	pub use etternaonline_api::{Error, v2::*};
}
use config::{Config, Data};

const CMD_TOP_HELP: &str = "Call this command with `+top[NN] [USERNAME] [SKILLSET]` (both params optional)";
const CMD_COMPARE_HELP: &str = "Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`";
const CMD_USERSET_HELP: &str = "Call this command with `+userset YOUR_EO_USERNAME`";
const CMD_RIVALSET_HELP: &str = "Call this command with `+rivalset YOUR_EO_USERNAME`";
const CMD_SCROLLSET_HELP: &str = "Call this command with `+scrollset [down/up]`";

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
	session: eo::Session,
}

impl State {
	pub fn load() -> anyhow::Result<Self> {
		let session = eo::Session::new_from_login(
			crate::auth::EO_USERNAME.to_owned(),
			crate::auth::EO_PASSWORD.to_owned(),
			crate::auth::EO_CLIENT_DATA.to_owned(),
			std::time::Duration::from_millis(1000),
			Some(std::time::Duration::from_millis(30000)),
		)?;

		Ok(State { session, config: Config::load(), data: Data::load() })
	}

	fn get_eo_username(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> anyhow::Result<String> {
		if let Some(eo_username) = self.data.eo_username(msg.author.id.0) {
			return Ok(eo_username.to_owned());
		}

		match self.session.user_details(&msg.author.name) {
			Ok(_) => Ok(msg.author.name.to_owned()),
			Err(eo::Error::UserNotFound) => {
				msg.channel_id.say(&ctx.http, &format!(
					"User '{}' not found on EO. Please manually specify your EtternaOnline \
						username with `+userset`",
					&msg.author.name
				))?;
				Err(anyhow::anyhow!("don't print this"))
			},
			Err(other) => Err(other.into()),
		}
	}

	fn top_scores(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		text: &str,
		mut limit: u32,
	) -> anyhow::Result<()> {
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
			None => self.session.user_top_10_scores(&eo_username),
			Some(skillset) => self.session.user_top_skillset_scores(&eo_username, skillset, limit),
		};
		if let Err(eo::Error::UserNotFound) = top_scores {
			msg.channel_id.say(&ctx.http, format!("No such user or skillset \"{}\"", eo_username))?;
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
	) -> anyhow::Result<()> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let latest_scores = self.session.user_latest_scores(&eo_username);
		if let Err(eo::Error::UserNotFound) = latest_scores {
			msg.channel_id.say(&ctx.http, format!("No such user \"{}\"", eo_username))?;
			return Ok(());
		}
		let latest_scores = latest_scores?;

		let country_code = self.session.user_details(&eo_username)?.country_code;

		let mut response = String::from("```");
		for (i, entry) in latest_scores.iter().enumerate() {
			response += &format!(
				"{}. {}: {:.2}x\n  ▸ Score: {:.2} Wife: {:.2}%\n",
				i + 1,
				&entry.song_name,
				entry.rate,
				entry.ssr_overall,
				entry.wifescore * 100.0,
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
	) -> anyhow::Result<()> {
		let eo_username = if text.is_empty() {
			self.get_eo_username(ctx, msg)?
		} else {
			text.to_owned()
		};

		let details = match self.session.user_details(&eo_username) {
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

		let ranks = self.session.user_ranks_per_skillset(&eo_username)?;

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
				.field(format!("About {}:", eo_username), html2md::parse_html(&details.about_me), false)
				.thumbnail(format!("https://etternaonline.com/avatars/{}", &details.avatar_url))
				.color(crate::ETTERNA_COLOR);
			if let Some(modifiers) = &details.default_modifiers {
				embed.field("Default modifiers:", modifiers, false);
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
	) -> anyhow::Result<()> {
		let scroll_type = if args.to_lowercase().starts_with("up") {
			pattern_visualize::ScrollType::Upscroll
		} else if args.starts_with("down") {
			pattern_visualize::ScrollType::Downscroll
		} else {
			self.data.scroll(msg.author.id.0).unwrap_or(pattern_visualize::ScrollType::Upscroll)
		};
		let bytes = pattern_visualize::generate(args, scroll_type)?;

		// Send the image into the channel where the summoning message comes from
		msg.channel_id.send_files(&ctx.http, vec![(bytes.as_slice(), "output.png")], |m| m)?;

		Ok(())
	}

	fn profile_compare(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		me: &str,
		you: &str,
	) -> anyhow::Result<()> {
		let me = self.session.user_details(&me)?;
		let you = self.session.user_details(you)?;

		let mut string = "```Prolog\n".to_owned();
		for skillset in eo::Skillset8::iter() {
			string += &format!(
				"{: >10}:   {: >5.2}  {}  {: >5.2}   {:+.2}\n",
				skillset.to_string(), // to_string, or the padding won't work
				me.rating.get(skillset),
				if (me.rating.get(skillset) - you.rating.get(skillset)).abs() < f64::EPSILON {
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

	fn command(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		cmd: &str,
		text: &str
	) -> anyhow::Result<()> {
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
			"quote" => {
				let quote_i = (rand::random::<f64>() * self.config.quotes.len() as f64) as usize;
				msg.channel_id.say(&ctx.http, &self.config.quotes[quote_i].quote)?;
			}
			"pattern" => {
				self.pattern(ctx, msg, text)?;
			},
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
				self.data.save()?;
				msg.channel_id.say(&ctx.http, &format!("Your scroll type is now {:?}", scroll))?;
			}
			"userset" => {
				if text.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_USERSET_HELP)?;
					return Ok(());
				}
				if let Err(e) = self.session.user_details(text) {
					if e == eo::Error::UserNotFound {
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
				self.data.save()?;
			},
			"rivalset" => {
				if text.is_empty() {
					msg.channel_id.say(&ctx.http, CMD_RIVALSET_HELP)?;
					return Ok(());
				}
				if let Err(eo::Error::UserNotFound) = self.session.user_details(text) {
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
				self.data.save()?;
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
	) -> anyhow::Result<()> {
		println!("Argh I really _want_ to show song info for {}, but the EO v2 API doesn't expose \
			the required functions :(", song_id);
		Ok(())
	}

	fn score_card(&mut self,
		ctx: &serenity::Context,
		msg: &serenity::Message,
		scorekey: &str,
	) -> anyhow::Result<()> {
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
	) -> anyhow::Result<()> {
		// Let's not do this, because if a non existing command is called (e.g. `+asdfg`) there'll
		// be typing broadcasted, but no actual response, which is stupid
		// if let Err(e) = msg.channel_id.broadcast_typing(&ctx.http) {
		// 	println!("Couldn't broadcast typing: {}", e);
		// }

		if msg.channel_id.0 == self.config.link_and_attachments_only_channel { // #work-in-progress
			let url_regex = regex::Regex::new(r"http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\(\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+").unwrap();
			let num_links = url_regex.find_iter(&msg.content).count();
			if num_links == 0 && msg.attachments.is_empty() {
				msg.delete(&ctx.http)?;
				let notice_msg = msg.channel_id.say(&ctx.http, "Only links and attachments are \
					allowed in this channel. For discussions use <#374775369330589696>")?;
				std::thread::sleep(std::time::Duration::from_millis(5000));
				notice_msg.delete(&ctx.http)?;
				return Ok(());
			}
		}

		// If the message is in etternaonline server, and not in an allowed channel, and not sent
		// by a person with the permission to manage the guild, don't process the command
		if let (Some(guild_id), Some(guild_member)) = (msg.guild_id, msg.member(&ctx.cache)) {
			if *guild_id.as_u64() == self.config.etterna_online_guild_id
				&& !self.config.allowed_channels.contains(msg.channel_id.as_u64())
				&& !guild_member.permissions(&ctx.cache)?.manage_guild()
			{
				return Ok(());
			}
		}

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

		if msg.content.starts_with('+') {
			let text = &msg.content[1..];

			// Split message into command part and parameter part
			let mut a = text.splitn(2, ' ');
			let command_name = a.next().unwrap().trim();
			let parameters = a.next().unwrap_or("").trim();
	
			self.command(&ctx, &msg, command_name, parameters)?;
		}

		Ok(())
	}

	pub fn guild_member_update(&mut self,
		ctx: serenity::Context,
		old: Option<serenity::Member>,
		new: serenity::Member
	) -> anyhow::Result<()> {
		let old = match old { Some(a) => a, None => return Ok(()) };
		
		let guild = new.guild_id.to_partial_guild(&ctx.http)?;
		
		let has_max_300_now = new.roles.iter()
			.any(|r| guild.roles[r].name.as_str() == "MAX 300");
		let had_max_300_previously = old.roles.iter()
			.any(|r| guild.roles[r].name.as_str() == "MAX 300");
		
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
}