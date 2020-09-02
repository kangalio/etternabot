# Etternabot
Bot for Etterna Online

## How to run
1. Download this repository
2. Enter the required credentials in src/auth.rs
2. _Note: if your server requires 2FA for moderation privileges - such as the EtternaOnline Discord - you need two-factor authentication enabled on your personal account_
3. Download the required optical character recognition models from [here](https://github.com/tesseract-ocr/tessdata_best) and [here](https://github.com/Shreeshrii/tessdata_shreetest):
    - eng.traineddata
    - digitsall_layer.traineddata
4. Place the downloaded OCR models into a new top-level directory called `ocr_data`
4. Run the bot with cargo
