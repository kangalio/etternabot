mod pattern_visualize;

use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::{
  Args, CommandResult,
  macros::command,
};

use crate::api::Api;

#[command]
pub fn user(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  let mut data = ctx.data.write();
  let token = data.get_mut::<crate::api::Api>().expect("expected api token in ShareMap.");
  let user = Api::get_user(&token, args.rest()).expect("User doesn't exist or there was a problem with the api");
  let reply = format!("{} {} ({})",
    user.attributes.userName,
    user.attributes.playerRating,
    user.r#type
  );
  msg.channel_id.say(&ctx.http, &reply)?;
  
  Ok(())
}

#[command]
pub fn pattern(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
  pattern_visualize::generate("noteskin.png", "output.png", &args.message())?;
  
  // Send the image into the channel where the summoning message comes from
  msg.channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;

  Ok(())
}