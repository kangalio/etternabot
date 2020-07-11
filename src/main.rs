mod commands;
mod auth;
mod api;

use serenity::{
  framework::{
    StandardFramework,
    standard::macros::group,
  },
  model::{event::ResumedEvent, gateway::Ready},
  prelude::*,
};
use log::{error, info};

use commands::{
  etterna::*,
  utils::*,
};
use api::*;

struct Handler;

impl EventHandler for Handler {
  fn ready(&self, _: Context, ready: Ready) {
    info!("Connected as {}", ready.user.name);
  }

  fn resume(&self, _: Context, _: ResumedEvent) {
    info!("Resumed");
  }
}

#[group]
#[commands(ping, user, pattern)]
struct General;

struct SessionKey;
impl TypeMapKey for SessionKey {
  type Value = api::Session;
}

fn main() {
  let token = auth::TOKEN;

  let mut client = Client::new(&token, Handler).expect("Unable to create client");

  {
    let mut data = client.data.write();
    data.insert::<SessionKey>(Session::login().expect("Invalid login credentials, probably"));
  }

  client.with_framework(StandardFramework::new()
    .configure(|c| c.prefix("~"))
    .group(&GENERAL_GROUP));

  if let Err(why) = client.start() {
    error!("Client error: {:?}", why);
  }
}
