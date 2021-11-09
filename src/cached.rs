use crate::{Context, Error};

pub struct Cached<T> {
	value: std::sync::Mutex<T>,
	last_update_time: std::sync::Mutex<Option<std::time::Instant>>,

	resource_name: &'static str,
	callback: fn(Context<'_>) -> poise::BoxFuture<'_, Result<T, Error>>,
	max_age: std::time::Duration,
}

impl<T: Default> Cached<T> {
	pub fn new(
		resource_name: &'static str,
		callback: fn(Context<'_>) -> poise::BoxFuture<'_, Result<T, Error>>,
		max_age: std::time::Duration,
	) -> Self {
		Self {
			value: std::sync::Mutex::new(T::default()),
			last_update_time: std::sync::Mutex::new(None),

			resource_name,
			callback,
			max_age,
		}
	}

	pub async fn fetch<'a>(&'a self, ctx: Context<'_>) -> std::sync::MutexGuard<'a, T> {
		let last_update_time = *self.last_update_time.lock().unwrap();
		let now = std::time::Instant::now();

		if let Some(last_update_time) = last_update_time {
			if now - last_update_time < self.max_age {
				return self.value.lock().unwrap();
			}
		}

		match last_update_time {
			Some(last_update_time) => log::info!(
				"{} were last updated {:#?} ago, updating them",
				self.resource_name,
				now - last_update_time,
			),
			None => log::info!(
				"{} haven't yet been updated, fetching them",
				self.resource_name
			),
		}

		let new_value = (self.callback)(ctx).await;
		let mut value = self.value.lock().unwrap();
		match new_value {
			Ok(new_value) => {
				*value = new_value;
				*self.last_update_time.lock().unwrap() = Some(now);
				log::info!("Finished updating {}", self.resource_name);
			}
			Err(e) => {
				log::warn!("Failed to update {}: {}", self.resource_name, e);
			}
		}

		value
	}
}
