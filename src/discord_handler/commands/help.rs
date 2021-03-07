use super::State;
use crate::{serenity, Error};

pub fn help(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	use rand::Rng as _;

	let embed_contents = if args.eq_ignore_ascii_case("pattern") {
		r#"
**+pattern [down/up] [NN]ths [noteskin] [zoom]x [keymode]k PATTERN STRING**
- `down/up` configures the scroll direction (note: you can set your default with `+scrollset`)
- `NNths` (e.g. `20ths`) sets the note snap. Can be used mid-pattern
- `noteskin` can be `delta-note`, `sbz`/`subtract-by-zero`, `dbz`/`divide-by-zero`, `mbz`/`multiply-by-zero`, `lambda`, or `wafles`/`wafles3`[.](https://pastebin.com/raw/5We1buQU)
- `zoom` (e.g. `2x`) applies a certain stretch to the notes
- `keymode` (e.g. `5k` can be used to force a certain keymode when it's not obvious

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
		let minanym = &state
			.config
			.minanyms
			.get(rand::thread_rng().gen_range(0, state.config.minanyms.len()))
			.unwrap();
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
*Print one of various random quotes, phrases and memes from various rhythm gaming communities ([Credit](https://github.com/ca25nada/spawncamping-wallhack/blob/master/Scripts/Quotes.lua))*
**+lastsession [username]**
*Show the last 10 scores*
**+help**
*Print this message*

You can also post links to scores and I will show info about them. If you add a judge (e.g. "J7") to
your message, I will also show the wifescores with that judge.
				"#,
			minanym,
		)
	};

	msg.channel_id.send_message(&ctx.http, |m| {
		m.embed(|e| e.description(embed_contents).color(crate::ETTERNA_COLOR))
	})?;
	Ok(())
}