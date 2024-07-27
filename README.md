# Etternabot
Bot for Etterna Online

## How to run
1. Install `fontconfig`, `fontconfig-devel` packages (adjust for non-Fedora distros)
1. Download this repository
1. Copy .env.example to .env and fill in the values
1. _Optional: Copy the config-default.json file to config.json and adjust the channels to your server_
  1. If you don't do this, those channel-specific features will not work
1. _Note: if your server requires 2FA for moderation privileges - such as the EtternaOnline Discord - you need two-factor authentication enabled on your personal account_
1. Run the bot with the required credentials using `cargo run`