use notifine::{create_trello_token, find_trello_token_by_telegram_user_id};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use teloxide::dispatching::dialogue;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dptree::case;
use teloxide::filter_command;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResultArticle, InputMessageContent,
    InputMessageContentText, KeyboardButton, KeyboardMarkup, Me, ParseMode,
};
use teloxide::utils::command::BotCommands;

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Init,
    BoardSelection,
    ListSelection,
    CardSelection,
    CardDetails,
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;

// create a hashmap to store the user's trello tokens

pub async fn run_trello_bot() {
    log::info!("Starting bot...");

    let bot = create_new_bot();
    // Bot::new(env::var("TRELLO_TELOXIDE_TOKEN").expect("TRELLO_TELOXIDE_TOKEN must be set"));

    let command_handler = filter_command::<Command, _>()
        .branch(case![Command::Start].endpoint(handle_start_command))
        .branch(case![Command::Help].endpoint(handle_help_command))
        .branch(case![Command::Testy].endpoint(handle_testy_command))
        .branch(case![Command::Login].endpoint(handle_login_command))
        .branch(case![Command::Boards].endpoint(handle_boards_command));

    let message_handler = dptree::entry().branch(
        Update::filter_message()
            .branch(command_handler)
            .branch(case![State::BoardSelection].endpoint(handle_board_selection))
            .branch(case![State::ListSelection].endpoint(handle_list_selection))
            // .branch(case![State::CardSelection].endpoint(handle_card_selection))
            // .branch(case![State::CardDetails].endpoint(handle_card_details))
            .branch(dptree::endpoint(handle_new_message)),
    );
    // .branch(Update::filter_my_chat_member().endpoint(handle_my_chat_member_update));

    let handler =
        dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    log::info!("Closing bot... Goodbye!");
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display this text")]
    Help,
    #[command(description = "Start")]
    Start,
    #[command(description = "Testy")]
    Testy,
    #[command(description = "Login")]
    Login,
    #[command(description = "Access")]
    Access,
    #[command(description = "Boards")]
    Boards,
}

/// Creates a keyboard made by buttons in a big column.
fn make_inline_keyboard() -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    let debian_versions = [
        "Buzz", "Rex", "Bo", "Hamm", "Slink", "Potato", "Woody", "Sarge", "Etch", "Lenny",
        "Squeeze", "Wheezy", "Jessie", "Stretch", "Buster", "Bullseye",
    ];

    for versions in debian_versions.chunks(3) {
        let row = versions
            .iter()
            .map(|&version| InlineKeyboardButton::callback(version.to_owned(), version.to_owned()))
            .collect();

        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

fn make_keyboard() -> KeyboardMarkup {
    let mut keyboard: Vec<Vec<KeyboardButton>> = vec![];

    let debian_versions = ["Buzz", "Duzz", "Fuzzy", "Huzzy"];

    for version in debian_versions {
        let button = vec![KeyboardButton::new(version.to_owned())];
        keyboard.push(button);
    }

    KeyboardMarkup::new(keyboard)
}

async fn handle_start_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    let keyboard = make_inline_keyboard();
    bot.send_message(msg.chat.id, "Debian versions:")
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_testy_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    let keyboard = make_keyboard();
    bot.send_message(msg.chat.id, "Debian versions:")
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_help_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;

    Ok(())
}

async fn handle_login_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    let request_URL = "https://trello.com/1/OAuthGetRequestToken";
    let authorize_URL = "https://trello.com/1/OAuthAuthorizeToken";
    let access_URL = "https://trello.com/1/OAuthGetAccessToken";

    let trello_key = env::var("TRELLO_KEY").expect("TRELLO_KEY must be set");
    let trello_secret = env::var("TRELLO_SECRET").expect("TRELLO_SECRET must be set");

    // base url + /trello/callback
    let callback = format!(
        "{}/trello/callback",
        env::var("WEBHOOK_BASE_URL").expect("WEBHOOK_BASE_URL must be set")
    );

    let client = oauth::Credentials::new(&trello_key, &trello_secret);

    let authorization_header = oauth::Builder::<_, _>::new(client, oauth::HmacSha1::new())
        .callback(callback.as_str())
        .post(request_URL, &());

    println!("Authorization header: {}", authorization_header);

    // send request to request_URL with authorization_header use ureq crate for this
    let response: String = ureq::post(request_URL)
        .set("Authorization", &authorization_header)
        .call()
        .unwrap()
        .into_string()?;

    // parse response to get oauth_token and oauth_token_secret
    let response: HashMap<String, String> = url::form_urlencoded::parse(response.as_bytes())
        .into_owned()
        .collect();

    let oauth_token = response.get("oauth_token").unwrap();
    let oauth_token_secret = response.get("oauth_token_secret").unwrap();

    // create new TrelloToken and save to db
    create_trello_token(oauth_token, oauth_token_secret, &msg.chat.id.to_string());

    println!("{oauth_token} and {oauth_token_secret}");

    let message = format!(
        "https://trello.com/1/OAuthAuthorizeToken\
                ?expiration=never&name=degusuk&scope=read%2Cwrite&\
                oauth_token={oauth_token}"
    );

    bot.send_message(msg.chat.id, message).await?;

    Ok(())
}

async fn handle_boards_command(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    // fetch boards of trello user then send them as make_keyboard
    let boards = get_trello_boards(&msg.chat.id.to_string());

    // map boards to buttons with their names
    let buttons: Vec<Vec<KeyboardButton>> = boards
        .iter()
        .map(|board| {
            log::info!("board: {}", board.id);
            log::info!("board: {}", board.name);
            vec![KeyboardButton::new(board.name.clone())]
        })
        .collect();

    let keyboard = KeyboardMarkup::new(buttons);
    dialogue.update(State::BoardSelection).await.unwrap();

    bot.send_message(msg.chat.id, "Your boards:")
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_board_selection(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
) -> ResponseResult<()> {
    log::info!("Board selection received");
    log::info!("Board name: {}", msg.text().unwrap());
    let chat_id = msg.chat.id;

    let boards = get_trello_boards(&msg.chat.id.to_string());
    // find board with name
    let board = boards
        .iter()
        .find(|board| board.name == msg.text().unwrap())
        .unwrap();

    log::info!("Board id: {}", board.id);

    // get lists of board
    let lists = get_trello_lists_of_board(&msg.chat.id.to_string(), &board.id);

    // map lists to buttons with their names
    let buttons: Vec<KeyboardButton> = lists
        .iter()
        .map(|list| {
            log::info!("listtt: {:?}", list);
            KeyboardButton::new(list.name.clone())
        })
        .collect();

    let buttons: Vec<Vec<KeyboardButton>> = vec![buttons];

    let keyboard = KeyboardMarkup::new(buttons).one_time_keyboard(true);

    dialogue.update(State::ListSelection).await.unwrap();

    bot.send_message(msg.chat.id, format!("Lists of {} board:", board.name))
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_list_selection(bot: Bot, dialogue: MyDialogue, msg: Message) -> ResponseResult<()> {
    log::info!("List selection received");
    log::info!("List name: {}", msg.text().unwrap());
    let chat_id = msg.chat.id;

    let keyboard = make_keyboard();

    dialogue.exit().await.unwrap();

    bot.send_message(msg.chat.id, "Horry versions:")
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

#[derive(oauth::Request)]
struct GetAccessToken<'a> {
    oauth_token: &'a str,
    oauth_verifier: &'a str,
}

async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let choose_debian_version = InlineQueryResultArticle::new(
        "0",
        "Chose debian version",
        InputMessageContent::Text(InputMessageContentText::new("Debian versions:")),
    )
    .reply_markup(make_inline_keyboard());

    bot.answer_inline_query(q.id, vec![choose_debian_version.into()])
        .await?;

    Ok(())
}

/// When it receives a callback from a button it edits the message with all
/// those buttons writing a text with the selected Debian version.
///
/// **IMPORTANT**: do not send privacy-sensitive data this way!!!
/// Anyone can read data stored in the callback button.
async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(version) = q.data {
        let text = format!("You chose: {version}");

        // Tell telegram that we've seen this query, to remove 🕑 icons from the
        //
        // clients. You could also use `answer_callback_query`'s optional
        // parameters to tweak what happens on the client side.
        bot.answer_callback_query(q.id).await?;

        // Edit text of the message to which the buttons were attached
        if let Some(Message { id, chat, .. }) = q.message {
            bot.edit_message_text(chat.id, id, text).await?;
        } else if let Some(id) = q.inline_message_id {
            bot.edit_message_text_inline(id, text).await?;
        }

        log::info!("You chose: {}", version);
    }

    Ok(())
}

pub async fn send_message_trello(chat_id: i64, message: String) -> ResponseResult<()> {
    log::info!("Sending message to {}: {}", chat_id, message);
    let bot = create_new_bot();

    let chat_id = ChatId(chat_id);

    bot.send_message(chat_id, message)
        .parse_mode(ParseMode::Html)
        .send()
        .await?;
    Ok(())
}

fn create_new_bot() -> Bot {
    Bot::new(env::var("TRELLO_TELOXIDE_TOKEN").expect("TRELLO_TELOXIDE_TOKEN must be set"))
}

#[derive(Deserialize)]
struct Board {
    id: String,
    name: String,
    // desc: String,
    // desc_data: String,
    // closed: bool,
    // id_member_creator: String,
    // pinned: bool,
    // url: String,
    // short_url: String,
    // memberships: String,
    // creation_method: String,
}

#[derive(Deserialize, Debug)]
struct List {
    id: String,
    name: String,
    pos: i64,
    closed: bool,
}

fn get_trello_boards(telegram_user_id: &str) -> Vec<Board> {
    let trello_token = find_trello_token_by_telegram_user_id(telegram_user_id).unwrap();

    let access_token = trello_token.access_token.unwrap();
    let trello_key = env::var("TRELLO_KEY").expect("TRELLO_KEY must be set");

    // use ureq for request to trello api
    let request_URL = "https://api.trello.com/1/members/me/boards";

    // let token =
    //     oauth::Token::from_parts(trello_key, trello_secret, access_token, access_token_secret);

    // let authorization_header =
    //     oauth::Builder::<_, _>::new(client, oauth::HmacSha1::new()).get(request_URL, &());

    // let authorization_header = oauth::get(request_URL, &(), &token, oauth::HmacSha1::new());

    let boards: Vec<Board> = match ureq::get(request_URL)
        // .set("Authorization", &authorization_header)
        .query("key", &trello_key)
        .query("token", &access_token)
        .call()
    {
        Ok(response) => {
            let boards: Vec<Board> = response.into_json().unwrap();
            boards
        }
        Err(error) => {
            println!("Error: {}", error);
            Vec::new()
        }
    };

    boards
}

fn get_trello_lists_of_board(telegram_user_id: &str, board_id: &str) -> Vec<List> {
    let trello_token = find_trello_token_by_telegram_user_id(telegram_user_id).unwrap();

    let access_token = trello_token.access_token.unwrap();
    let trello_key = env::var("TRELLO_KEY").expect("TRELLO_KEY must be set");
    // use ureq for request to trello api
    let request_url = format!("https://api.trello.com/1/boards/{board_id}/lists");

    let lists: Vec<List> = match ureq::get(&request_url)
        .query("key", &trello_key)
        .query("token", &access_token)
        .call()
    {
        Ok(response) => {
            let lists: Vec<List> = response.into_json().unwrap();
            lists
        }
        Err(error) => {
            println!("Error: {error}");
            Vec::new()
        }
    };

    lists
}

async fn handle_new_message(bot: Bot, message: Message) -> ResponseResult<()> {
    let chat_id = message.chat.id.0;

    if let Some(text) = message.text() {
        log::info!("Received message from {}: {}", chat_id, text);
    }

    log::warn!("{:#?}", message.via_bot);
    Ok(())
}
