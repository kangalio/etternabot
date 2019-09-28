use serenity::client::Client;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::gateway::Activity;
use serenity::model::user::OnlineStatus;
use serenity::prelude::{EventHandler, Context};
use serenity::framework::standard::{
    StandardFramework,
    CommandResult,
    Args,
    macros::{
        command,
        group
    }
};

mod api;
mod config;

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;

lazy_static! {
    static ref KEY: String = api::login()
        .expect("Unable to log in to Etterna Online api");
}

group!({
    name: "general",
    options: {},
    commands: [user],
});

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        println!("Connnected as {}#{}", ready.user.name, ready.user.discriminator);
        ctx.set_presence(Some(Activity::playing("Etterna")), OnlineStatus::DoNotDisturb);
    }

    fn message(&self, _: Context, msg: Message) {
        println!("{}#{}: {}", msg.author.name, msg.author.discriminator, msg.content);
    }
}

fn main() {
    let mut client = Client::new(&config::token, Handler)
        .expect("Error creating client");
    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("~"))
        .group(&GENERAL_GROUP));

    if let Err(why) = client.start() {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
fn user(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let data = api::get_user(&KEY, &args.rest());
    match data {
        Ok(u) => {
            let user = u;
            let reply = format!("{} {} ({})",
                                    user.attributes.userName,
                                    user.attributes.playerRating,
                                    user.r#type);
            msg.reply(ctx, &reply)?;

            Ok(())
        },
        Err(why) => {
            println!("{:?}", why);
            Ok(())
        }
    }
}