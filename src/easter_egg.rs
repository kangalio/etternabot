// TODO: support simultaneous casing and char changes

fn is_scream(s: &str) -> bool {
	s.bytes().all(|b| !b.is_ascii_lowercase())
}

fn make_scream(s: &mut String) {
	s.make_ascii_uppercase()
}

fn is_spongebob(s: &str) -> bool {
	// Filter out tiny strings and "Xx" which is title case not spongebob
	let (first, rest) = match s.as_bytes() {
		[] | [_] => return false,
		[a, b] if a.is_ascii_uppercase() && b.is_ascii_lowercase() => return false,
		[first, rest @ ..] => (first, rest),
	};

	let mut prev_was_uppercase = first.is_ascii_uppercase();
	for c in rest {
		if prev_was_uppercase == c.is_ascii_uppercase() {
			return false;
		}
		prev_was_uppercase = c.is_ascii_uppercase();
	}
	true
}

fn make_spongebob(s: &mut String) {
	let mut uppercase = true;
	*s = s
		.chars()
		.map(|c| {
			uppercase = !uppercase;
			match uppercase {
				true => c.to_ascii_uppercase(),
				false => c.to_ascii_lowercase(),
			}
		})
		.collect()
}

fn is_reverse_scream(s: &str) -> bool {
	match s.as_bytes() {
		[first, rest @ ..] if rest.len() > 0 => {
			first.is_ascii_lowercase() && rest.iter().all(|c| c.is_ascii_uppercase())
		}
		_ => false,
	}
}

fn make_reverse_scream(s: &mut String) {
	fn transform_single_word(s: &str) -> String {
		let mut is_first = true;
		s.chars()
			.map(|c| {
				if is_first {
					is_first = false;
					c.to_ascii_lowercase()
				} else {
					c.to_ascii_uppercase()
				}
			})
			.collect()
	}

	let words = s
		.split(|c: char| !c.is_alphabetic())
		.filter(|&s| s != "")
		.map(|word| {
			let index = word.as_ptr() as usize - s.as_ptr() as usize;
			let transformed = transform_single_word(word);
			(index, transformed)
		})
		.collect::<Vec<_>>();

	for (index, transformed) in words {
		s.replace_range(index..index + transformed.len(), &transformed);
	}
}

pub fn casing_transformer(template: &str) -> Option<fn(&mut String)> {
	if is_scream(template) {
		Some(make_scream)
	} else if is_spongebob(template) {
		Some(make_spongebob)
	} else if is_reverse_scream(template) {
		Some(make_reverse_scream)
	} else {
		None
	}
}

const NORMAL_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
// Sourced from https://qaz.wtf/u/convert.cgi
const FUNKY_ALPHABETS: &[&str] = &[
    "ⓐⓑⓒⓓⓔⓕⓖⓗⓘⓙⓚⓛⓜⓝⓞⓟⓠⓡⓢⓣⓤⓥⓦⓧⓨⓩⒶⒷⒸⒹⒺⒻⒼⒽⒾⒿⓀⓁⓂⓃⓄⓅⓆⓇⓈⓉⓊⓋⓌⓍⓎⓏ",
    "🅐🅑🅒🅓🅔🅕🅖🅗🅘🅙🅚🅛🅜🅝🅞🅟🅠🅡🅢🅣🅤🅥🅦🅧🅨🅩🅐🅑🅒🅓🅔🅕🅖🅗🅘🅙🅚🅛🅜🅝🅞🅟🅠🅡🅢🅣🅤🅥🅦🅧🅨🅩",
    "ａｂｃｄｅｆｇｈｉｊｋｌｍｎｏｐｑｒｓｔｕｖｗｘｙｚＡＢＣＤＥＦＧＨＩＪＫＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ",
    "𝐚𝐛𝐜𝐝𝐞𝐟𝐠𝐡𝐢𝐣𝐤𝐥𝐦𝐧𝐨𝐩𝐪𝐫𝐬𝐭𝐮𝐯𝐰𝐱𝐲𝐳𝐀𝐁𝐂𝐃𝐄𝐅𝐆𝐇𝐈𝐉𝐊𝐋𝐌𝐍𝐎𝐏𝐐𝐑𝐒𝐓𝐔𝐕𝐖𝐗𝐘𝐙",
    "𝖆𝖇𝖈𝖉𝖊𝖋𝖌𝖍𝖎𝖏𝖐𝖑𝖒𝖓𝖔𝖕𝖖𝖗𝖘𝖙𝖚𝖛𝖜𝖝𝖞𝖟𝕬𝕭𝕮𝕯𝕰𝕱𝕲𝕳𝕴𝕵𝕶𝕷𝕸𝕹𝕺𝕻𝕼𝕽𝕾𝕿𝖀𝖁𝖂𝖃𝖄𝖅",
    "𝒂𝒃𝒄𝒅𝒆𝒇𝒈𝒉𝒊𝒋𝒌𝒍𝒎𝒏𝒐𝒑𝒒𝒓𝒔𝒕𝒖𝒗𝒘𝒙𝒚𝒛𝑨𝑩𝑪𝑫𝑬𝑭𝑮𝑯𝑰𝑱𝑲𝑳𝑴𝑵𝑶𝑷𝑸𝑹𝑺𝑻𝑼𝑽𝑾𝑿𝒀𝒁",
    "𝓪𝓫𝓬𝓭𝓮𝓯𝓰𝓱𝓲𝓳𝓴𝓵𝓶𝓷𝓸𝓹𝓺𝓻𝓼𝓽𝓾𝓿𝔀𝔁𝔂𝔃𝓐𝓑𝓒𝓓𝓔𝓕𝓖𝓗𝓘𝓙𝓚𝓛𝓜𝓝𝓞𝓟𝓠𝓡𝓢𝓣𝓤𝓥𝓦𝓧𝓨𝓩",
    "𝕒𝕓𝕔𝕕𝕖𝕗𝕘𝕙𝕚𝕛𝕜𝕝𝕞𝕟𝕠𝕡𝕢𝕣𝕤𝕥𝕦𝕧𝕨𝕩𝕪𝕫𝔸𝔹ℂ𝔻𝔼𝔽𝔾ℍ𝕀𝕁𝕂𝕃𝕄ℕ𝕆ℙℚℝ𝕊𝕋𝕌𝕍𝕎𝕏𝕐ℤ",
    "𝚊𝚋𝚌𝚍𝚎𝚏𝚐𝚑𝚒𝚓𝚔𝚕𝚖𝚗𝚘𝚙𝚚𝚛𝚜𝚝𝚞𝚟𝚠𝚡𝚢𝚣𝙰𝙱𝙲𝙳𝙴𝙵𝙶𝙷𝙸𝙹𝙺𝙻𝙼𝙽𝙾𝙿𝚀𝚁𝚂𝚃𝚄𝚅𝚆𝚇𝚈𝚉",
    "𝖺𝖻𝖼𝖽𝖾𝖿𝗀𝗁𝗂𝗃𝗄𝗅𝗆𝗇𝗈𝗉𝗊𝗋𝗌𝗍𝗎𝗏𝗐𝗑𝗒𝗓𝖠𝖡𝖢𝖣𝖤𝖥𝖦𝖧𝖨𝖩𝖪𝖫𝖬𝖭𝖮𝖯𝖰𝖱𝖲𝖳𝖴𝖵𝖶𝖷𝖸𝖹",
    "𝗮𝗯𝗰𝗱𝗲𝗳𝗴𝗵𝗶𝗷𝗸𝗹𝗺𝗻𝗼𝗽𝗾𝗿𝘀𝘁𝘂𝘃𝘄𝘅𝘆𝘇𝗔𝗕𝗖𝗗𝗘𝗙𝗚𝗛𝗜𝗝𝗞𝗟𝗠𝗡𝗢𝗣𝗤𝗥𝗦𝗧𝗨𝗩𝗪𝗫𝗬𝗭",
    "𝙖𝙗𝙘𝙙𝙚𝙛𝙜𝙝𝙞𝙟𝙠𝙡𝙢𝙣𝙤𝙥𝙦𝙧𝙨𝙩𝙪𝙫𝙬𝙭𝙮𝙯𝘼𝘽𝘾𝘿𝙀𝙁𝙂𝙃𝙄𝙅𝙆𝙇𝙈𝙉𝙊𝙋𝙌𝙍𝙎𝙏𝙐𝙑𝙒𝙓𝙔𝙕",
    "𝘢𝘣𝘤𝘥𝘦𝘧𝘨𝘩𝘪𝘫𝘬𝘭𝘮𝘯𝘰𝘱𝘲𝘳𝘴𝘵𝘶𝘷𝘸𝘹𝘺𝘻𝘈𝘉𝘊𝘋𝘌𝘍𝘎𝘏𝘐𝘑𝘒𝘓𝘔𝘕𝘖𝘗𝘘𝘙𝘚𝘛𝘜𝘝𝘞𝘟𝘠𝘡",
    "⒜⒝⒞⒟⒠⒡⒢⒣⒤⒥⒦⒧⒨⒩⒪⒫⒬⒭⒮⒯⒰⒱⒲⒳⒴⒵⒜⒝⒞⒟⒠⒡⒢⒣⒤⒥⒦⒧⒨⒩⒪⒫⒬⒭⒮⒯⒰⒱⒲⒳⒴⒵",
    "🇦🇧🇨🇩🇪🇫🇬🇭🇮🇯🇰🇱🇲🇳🇴🇵🇶🇷🇸🇹🇺🇻🇼🇽🇾🇿🇦🇧🇨🇩🇪🇫🇬🇭🇮🇯🇰🇱🇲🇳🇴🇵🇶🇷🇸🇹🇺🇻🇼🇽🇾🇿",
    "🄰🄱🄲🄳🄴🄵🄶🄷🄸🄹🄺🄻🄼🄽🄾🄿🅀🅁🅂🅃🅄🅅🅆🅇🅈🅉🄰🄱🄲🄳🄴🄵🄶🄷🄸🄹🄺🄻🄼🄽🄾🄿🅀🅁🅂🅃🅄🅅🅆🅇🅈🅉",
    "🅰🅱🅲🅳🅴🅵🅶🅷🅸🅹🅺🅻🅼🅽🅾🅿🆀🆁🆂🆃🆄🆅🆆🆇🆈🆉🅰🅱🅲🅳🅴🅵🅶🅷🅸🅹🅺🅻🅼🅽🅾🅿🆀🆁🆂🆃🆄🆅🆆🆇🆈🆉",
    "ábćdéfǵhíjḱĺḿńőṕqŕśtúvẃxӳźÁBĆDÉFǴHíJḰĹḾŃŐṔQŔśTŰVẂXӲŹ",
    "ﾑ乃cd乇ｷgんﾉﾌズﾚﾶ刀oｱq尺丂ｲu√wﾒﾘ乙ﾑ乃cd乇ｷgんﾉﾌズﾚﾶ刀oｱq尺丂ｲu√wﾒﾘ乙",
    "ค๒ƈɗﻉिﻭɦٱﻝᛕɭ๓กѻρ۹ɼรՇપ۷ฝซץչค๒ƈɗﻉिﻭɦٱﻝᛕɭ๓กѻρ۹ɼรՇપ۷ฝซץչ",
    "αв¢∂єƒﻭнιנкℓмησρ۹яѕтυνωχуչαв¢∂єƒﻭнιנкℓмησρ۹яѕтυνωχуչ",
    "ค๒ς๔єŦﻮђเןкɭ๓ภ๏קợгรՇยשฬאץչค๒ς๔єŦﻮђเןкɭ๓ภ๏קợгรՇยשฬאץչ",
    "аъсↁэfБЂіјкlмиорqѓѕтцvшхЎzДБҀↁЄFБНІЈЌLМИФРQЯЅГЦVЩЖЧZ",
    "ልጌርዕቿቻኗዘጎጋጕረጠክዐየዒዪነፕሁሀሠሸሃጊልጌርዕቿቻኗዘጎጋጕረጠክዐየዒዪነፕሁሀሠሸሃጊ",
    "𝔞𝔟𝔠𝔡𝔢𝔣𝔤𝔥𝔦𝔧𝔨𝔩𝔪𝔫𝔬𝔭𝔮𝔯𝔰𝔱𝔲𝔳𝔴𝔵𝔶𝔷𝔄𝔅ℭ𝔇𝔈𝔉𝔊ℌℑ𝔍𝔎𝔏𝔐𝔑𝔒𝔓𝔔ℜ𝔖𝔗𝔘𝔙𝔚𝔛𝔜ℨ",
    "äḅċḋëḟġḧïjḳḷṁṅöṗqṛṡẗüṿẅẍÿżÄḄĊḊЁḞĠḦЇJḲḶṀṄÖṖQṚṠṪÜṾẄẌŸŻ",
    "ᴀʙᴄᴅᴇꜰɢʜɪᴊᴋʟᴍɴᴏᴩqʀꜱᴛᴜᴠᴡxyᴢᴀʙᴄᴅᴇꜰɢʜɪᴊᴋʟᴍɴᴏᴩQʀꜱᴛᴜᴠᴡxYᴢ",
    "ȺƀȼđɇfǥħɨɉꝁłmnøᵽꝗɍsŧᵾvwxɏƶȺɃȻĐɆFǤĦƗɈꝀŁMNØⱣꝖɌSŦᵾVWXɎƵ",
    "ₐbcdₑfgₕᵢⱼₖₗₘₙₒₚqᵣₛₜᵤᵥwₓyzₐBCDₑFGₕᵢⱼₖₗₘₙₒₚQᵣₛₜᵤᵥWₓYZ",
    "ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖqʳˢᵗᵘᵛʷˣʸᶻᴬᴮᶜᴰᴱᶠᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾQᴿˢᵀᵁⱽᵂˣʸᶻ",
    "ɐqɔpǝɟƃɥıɾʞןɯuodbɹsʇnʌʍxʎzꓯꓭↃꓷƎℲ⅁ɥıᒋꓘ⅂WuOꓒÒꓤSꓕꓵ𐌡MX⅄Z",
    "AdↄbɘꟻgHijklmᴎoqpᴙꙅTUvwxYzAdↃbƎꟻGHIJK⅃MᴎOꟼpᴙꙄTUVWXYZ",
];

pub struct CharTransformer(pub Box<dyn Fn(&mut String) + Send + Sync>);

// Using generic C instead of poise::Command to keep this module pure and innocent from outside
// types
pub fn char_transformer<C>(
	s: &str,
	find_command: impl Fn(&str) -> Option<C>,
) -> Option<(C, CharTransformer)> {
	let reversed = s.chars().rev().collect::<String>();
	if let Some(command) = find_command(&reversed) {
		let transformer = |s: &mut String| *s = s.chars().rev().collect::<String>();
		return Some((command, CharTransformer(Box::new(transformer))));
	}

	for &funky_alphabet in FUNKY_ALPHABETS {
		if let Some(untransformed) = s
			.chars()
			.map(|c| {
				funky_alphabet
					.chars()
					.position(|x| x == c)
					.map(|i| NORMAL_ALPHABET.as_bytes()[i] as char)
			})
			.collect::<Option<String>>()
		{
			println!("{} => {}", s, untransformed);
			if let Some(command) = find_command(&untransformed) {
				let transformer = move |s: &mut String| {
					*s = s
						.chars()
						.map(|c| match NORMAL_ALPHABET.find(c) {
							// unwrap_or case shouldn't happen (all alphabets are same length)
							Some(i) => funky_alphabet.chars().nth(i).unwrap_or(' '),
							None => c,
						})
						.collect()
				};
				return Some((command, CharTransformer(Box::new(transformer))));
			}
			break;
		}
	}

	None
}
