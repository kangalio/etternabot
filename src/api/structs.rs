#![allow(non_snake_case)]
#[derive(Deserialize, Debug)]
pub struct UserData {
    pub data: Option<User>,
    pub errors: Option<Vec<EOError>>
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub r#type: String,
    pub id: String,
    pub attributes: Attributes
}

#[derive(Deserialize, Debug)]
pub struct Attributes {
    pub userName: String,
    pub aboutMe: String,
    pub moderator: bool,
    pub patreon: bool,
    pub avatar: String,
    pub countryCode: String,
    pub playerRating: f64,
    pub defaultModifiers: String,
    pub skillsets: Skillsets
}

#[derive(Deserialize, Debug)]
pub struct Skillsets {
    pub Stream: f64,
    pub Jumpstream: f64,
    pub Handstream: f64,
    pub Stamina: f64,
    pub JackSpeed: f64,
    pub Chordjack: f64,
    pub Technical: f64
}

#[derive(Deserialize, Debug)]
pub struct Login {
    pub data: Option<LoginData>,
    pub errors: Option<Vec<EOError>>
}

#[derive(Deserialize, Debug)]
pub struct EOError {
    pub status: u32,
    pub title: String,
    pub detail: String
}

#[derive(Deserialize, Debug)]
pub struct LoginData {
    pub r#type: String,
    pub id: String,
    pub attributes: LoginAttributes,
}

#[derive(Deserialize, Debug)]
pub struct LoginAttributes {
    pub accessToken: String,
    pub expiresAt: i32
}