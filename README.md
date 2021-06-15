# Etternabot
Bot for Etterna Online

## How to run
1. Download this repository
1. _Optional: Copy the config-default.json file to config.json and adjust the channels to your server_
  1. If you don't do this, those channel-specific features will not work
1. _Note: if your server requires 2FA for moderation privileges - such as the EtternaOnline Discord - you need two-factor authentication enabled on your personal account_
1. Run the bot with the required credentials using `IMGBB_API_KEY=... DISCORD_BOT_TOKEN=... EO_USERNAME=... EO_PASSWORD=... EO_API_KEY=... EO_CLIENT_DATA=... cargo run`
  - `IMGBB_API_KEY` can be obtained at https://api.imgbb.com/ and is required to post the skillgraph when invoked as a slash command
  - `DISCORD_BOT_TOKEN` is the token of your registered Discord bot
  - `EO_USERNAME` and `EO_PASSWORD` are the credentials of any valid EtternaOnline user account. Required by EtternaOnline for v2 API login
  - `EO_API_KEY` is required for v1 API login
  - `EO_CLIENT_DATA` is required for v2 API login
