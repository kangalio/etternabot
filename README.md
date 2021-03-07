# Etternabot
Bot for Etterna Online

## How to run
1. Download this repository
1. _Note: if your server requires 2FA for moderation privileges - such as the EtternaOnline Discord - you need two-factor authentication enabled on your personal account_
1. Download the required optical character recognition models from [here](https://github.com/tesseract-ocr/tessdata_best) and [here](https://github.com/Shreeshrii/tessdata_shreetest):
    - eng.traineddata
    - digitsall_layer.traineddata
1. Place the downloaded OCR models into a new top-level directory called `ocr_data`
1. Run the bot with the required credentials using `DISCORD_BOT_TOKEN=... EO_USERNAME=... EO_PASSWORD=... EO_CLIENT_DATA=... cargo run`
