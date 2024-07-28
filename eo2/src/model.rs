use serde::Deserialize;

fn deserialize_wife<'de, D>(deserializer: D) -> Result<etterna::Wifescore, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let raw = f32::deserialize(deserializer)?;

	etterna::Wifescore::from_percent(raw).ok_or(serde::de::Error::custom(format!(
		"invalid wifescore percent: {}",
		raw
	)))
}

fn deserialize_scorekey<'de, D>(deserializer: D) -> Result<etterna::Scorekey, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let raw = String::deserialize(deserializer)?;

	etterna::Scorekey::new(raw.clone()).ok_or(serde::de::Error::custom(format!(
		"invalid wifescore percent: {}",
		raw,
	)))
}

fn deserialize_rate<'de, D>(deserializer: D) -> Result<etterna::Rate, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let raw = f32::deserialize(deserializer)?;

	etterna::Rate::from_f32(raw.clone())
		.ok_or(serde::de::Error::custom(format!("invalid rate: {}", raw,)))
}

fn deserialize_stringified_f32<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let raw = String::deserialize(deserializer)?;

	raw.parse().map_err(serde::de::Error::custom)
}

#[derive(serde::Deserialize)]
pub struct Score {
	pub id: u32,
	#[serde(deserialize_with = "deserialize_scorekey")]
	pub key: etterna::Scorekey,

	#[serde(flatten)]
	pub ssr: Skillsets8,
	/// Max = 100.0
	#[serde(deserialize_with = "deserialize_wife")]
	pub wife: etterna::Wifescore,
	// pub combo: u32,              // 579
	// pub valid: AAA,              // true
	// pub modifiers: AAA,          // "C913, Reverse, Mirror, Overhead, Eliminate294"
	// pub marvelous: AAA,          // 5982
	// pub perfect: AAA,            // 211
	// pub great: AAA,              // 62
	// pub good: AAA,               // 2
	// pub bad: AAA,                // 2
	// pub miss: AAA,               // 52
	// pub hit_mine: AAA,           // 0
	// pub held: AAA,               // 192
	// pub let_go: AAA,             // 0
	// pub missed_hold: AAA,        // 0
	#[serde(deserialize_with = "deserialize_rate")]
	pub rate: etterna::Rate,
	pub datetime: String, // "2024-07-20 02:39:49"
	// pub replay: AAA,             // true
	// pub chord_cohesion: AAA,     // false
	// pub calculator_version: AAA, // 511
	// pub top_score: AAA,          // 1
	// pub wife_version: AAA,       // 3
	// pub judge: AAA,              // "J4"
	// pub grade: AAA,              // "AA"
	// pub song: AAA,
	// pub chart: AAA, // {"id": 351172, "key": "Xe27f0b333f177b1e835942593fba7d75878dd23d", "difficulty": "Challenge", "short": "IN", "favorite_count": 0, "keys": 4, "overall": "39.988414764404", "rates": [{"rate": 1}]}
	// pub user: AAA, // {"username": "TravisBickle", "created_at": "2024-05-16T21:19:12.000000Z", "bio": "respect players:\nmeatloaf2654\n[Cryptonic](https://www.youtube.com/@CryptonicLive)\nkittieside\nkyionining\ncaughtintheweb", "country": "US", "avatar": "https://storage.etternaonline.com/images/922678/conversions/ca9cb81019c45ed5445e7761b3e9675d-optimised.webp", "avatar_thumb": "https://storage.etternaonline.com/images/922678/conversions/ca9cb81019c45ed5445e7761b3e9675d-thumb.webp", "overall": "38.891601562500", "roles": [], "preferences": ["showTranslit", "nsfw"], "supporter": false, "rank": 1, "skillset_ranks": {"stream": 9, "jumpstream": 4, "handstream": 2, "jacks": 1, "chordjacks": 1, "stamina": 1, "technical": 5}, "banned": false, "stream": "35.997558593750", "jumpstream": "36.276855468750", "handstream": "37.527343750000", "jacks": "38.454101562500", "chordjacks": "41.297851562500", "stamina": "40.980468750000", "technical": "36.099121093750"}
	pub song: Song,
}

#[derive(serde::Deserialize)]
pub struct Song {
	pub name: String,
}

#[derive(serde::Deserialize)]
pub struct User {
	pub username: String,
	pub bio: String,
	pub country: String,
	pub rank: u32,
	pub skillset_ranks: SkillsetRanks7,
	#[serde(flatten)]
	pub skillsets: Skillsets8,
	pub roles: Vec<String>,
	pub supporter: bool,
	pub avatar: String,
}

impl User {
	pub fn rank(&self) -> etterna::UserRank {
		etterna::UserRank {
			overall: self.rank,
			stream: self.skillset_ranks.stream,
			jumpstream: self.skillset_ranks.jumpstream,
			handstream: self.skillset_ranks.handstream,
			jackspeed: self.skillset_ranks.jacks,
			chordjack: self.skillset_ranks.chordjacks,
			stamina: self.skillset_ranks.stamina,
			technical: self.skillset_ranks.technical,
		}
	}
}

#[derive(serde::Deserialize)]
pub struct SkillsetRanks7 {
	pub stream: u32,
	pub jumpstream: u32,
	pub handstream: u32,
	pub jacks: u32,
	pub chordjacks: u32,
	pub stamina: u32,
	pub technical: u32,
}

#[derive(serde::Deserialize)]
pub struct Skillsets8 {
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub overall: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub stream: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub jumpstream: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub handstream: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub jacks: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub chordjacks: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub stamina: f32,
	#[serde(deserialize_with = "deserialize_stringified_f32")]
	pub technical: f32,
}

impl Skillsets8 {
	pub fn skillsets8(&self) -> etterna::Skillsets8 {
		etterna::Skillsets8 {
			overall: self.overall,
			stream: self.stream,
			jumpstream: self.jumpstream,
			handstream: self.handstream,
			stamina: self.stamina,
			jackspeed: self.jacks,
			chordjack: self.chordjacks,
			technical: self.technical,
		}
	}

	pub fn skillsets7(&self) -> etterna::Skillsets7 {
		self.skillsets8().to_skillsets7()
	}
}
