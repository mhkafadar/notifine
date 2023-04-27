use crate::bots::gitlab_bot::send_message_gitlab;
use crate::bots::trello_bot::send_message_trello;
use actix_web::{get, web, HttpResponse, Responder};
use notifine::{find_trello_token_by_token_key, update_trello_token_access_token};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use ureq::Error;

#[derive(Deserialize)]
pub struct CallbackQueryParams {
    oauth_token: String,
    oauth_verifier: String,
}

#[derive(oauth::Request)]
struct GetAccessToken<'a> {
    oauth_verifier: &'a str,
}

// #[derive(Deserialize)]
// struct AccessTokenResponse {
//     oauth_token: String,
//     oauth_token_secret: String,
// }

#[get("/trello/callback")]
// handle CallbackQueryParams
pub async fn handle_trello_callback(
    query_params: web::Query<CallbackQueryParams>,
) -> impl Responder {
    log::info!("Trello callback");
    log::info!("oauth_token: {}", query_params.oauth_token);
    log::info!("oauth_verifier: {}", query_params.oauth_verifier);

    // get trellotoken by oauth_token from db
    let trello_token = find_trello_token_by_token_key(&query_params.oauth_token).unwrap();

    let trello_key = env::var("TRELLO_KEY").expect("TRELLO_KEY must be set");
    let trello_secret = env::var("TRELLO_SECRET").expect("TRELLO_SECRET must be set");

    let token = oauth::Token::from_parts(
        trello_key,
        trello_secret,
        query_params.oauth_token.clone(),
        trello_token.token_secret.as_ref().unwrap().to_string(),
    );

    let access_url = "https://trello.com/1/OAuthGetAccessToken";
    // generate auth header

    let request = GetAccessToken {
        oauth_verifier: &query_params.oauth_verifier,
    };

    let authorization_header = oauth::get(access_url, &request, &token, oauth::HmacSha1::new());

    println!("Authorization header: {}", authorization_header);

    // send request to request_URL with authorization_header use ureq crate for this
    // set header application/x-www-form-urlencoded

    match ureq::get(access_url)
        .set("Authorization", &authorization_header)
        .query("oauth_verifier", &query_params.oauth_verifier)
        // .send_json(ureq::json!({
        //     "oauth_token": request.oauth_token,
        //     "oauth_verifier": request.oauth_verifier
        // }))
        .call()
    {
        Ok(response) => {
            // parse response to get oauth_token and oauth_token_secret
            let parsed_response: HashMap<String, String> =
                url::form_urlencoded::parse(response.into_string().unwrap().as_bytes())
                    .into_owned()
                    .collect();

            let oauth_token = parsed_response.get("oauth_token").unwrap();
            let oauth_token_secret = parsed_response.get("oauth_token_secret").unwrap();
            // update trello_token in db
            update_trello_token_access_token(&trello_token, oauth_token, oauth_token_secret);

            send_message_trello(
                trello_token
                    .telegram_user_id
                    .as_ref()
                    .unwrap()
                    .parse::<i64>()
                    .unwrap(),
                "You have successfuly connected your trello account!".to_string(),
            )
            .await
            .unwrap();
        }
        Err(Error::Status(code, response)) => {
            println!("Error: {}", response.into_string().unwrap());
        }
        Err(_) => {
            println!("Error!!");
        }
    }

    // println!("Response: {}", response);

    HttpResponse::Ok() //
}
