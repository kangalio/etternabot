mod wife2;
pub use wife2::*;
mod wife3;
pub use wife3::*;


const INNER_STRAY_WEIGHT: f32 = -5.5; // this is an extension from me
pub const STRAY_WEIGHT: f32 = INNER_STRAY_WEIGHT / 2.0;

pub trait Wife {
	const MINE_HIT_WEIGHT: f32;
	const HOLD_DROP_WEIGHT: f32;
	const MISS_WEIGHT: f32;

	fn calc(deviation: f32) -> f32;

	// Misses must be present in the `deviations` slice in form of a `1.000000` value
	fn apply(deviations: &[f32], num_mine_hits: u64, num_hold_drops: u64) -> f32 {
		let mut wifescore_sum = 0.0;
		for &deviation in deviations {
			if (deviation - 1.0).abs() < 0.0001 { // it's a miss
				wifescore_sum += Self::MISS_WEIGHT;
			} else {
				wifescore_sum += Self::calc(deviation);
			}
		}

		wifescore_sum += num_mine_hits as f32 * Self::MINE_HIT_WEIGHT;
		wifescore_sum += num_hold_drops as f32 * Self::HOLD_DROP_WEIGHT;

		wifescore_sum / deviations.len() as f32
	}
}

pub fn wife2(deviation: f32) -> f32 { Wife2::calc(deviation) }
pub fn wife3(deviation: f32) -> f32 { Wife3::calc(deviation) }