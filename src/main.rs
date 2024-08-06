use std::{collections::VecDeque, sync::Arc};

use async_openai::{
    config::OpenAIConfig,
    error::OpenAIError,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use teloxide::prelude::*;
use tokio::sync::Mutex;

const PROMPT: &str = "You are a member of a Telegram group chat between friends. The conversation is casual. \
Everyone speaks Russian and you are expected to speak Russian too. You need to support the conversation by replying with your opinion. Joke sometimes. Use slang. \
Act natural, match the style of the conversation, like punctuation, capitalization, and emojis. Be very short but to the point. Make it no longer than 3 sentences. Appear as if you know \
a lot about the topic. Pretend to be an expert. Use Russian swear words like сука or блять where appropriate, when you're disappointed.";

async fn request_completion(
    cli: Client<OpenAIConfig>,
    conv: &VecDeque<String>,
) -> Result<String, OpenAIError> {
    let mut messages = conv
        .iter()
        .map(|msg| {
            ChatCompletionRequestUserMessageArgs::default()
                .content(msg.as_ref())
                .build()
                .map(|msg| msg.into())
        })
        .collect::<Result<VecDeque<ChatCompletionRequestMessage>, OpenAIError>>()?;

    messages.push_front(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(PROMPT)
            .build()?
            .into(),
    );

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u16)
        .model("gpt-4o")
        .messages(messages)
        .build()?;

    let response = cli.chat().create(request).await?;
    let message = response
        .choices
        .first()
        .map(|choice| choice.message.content.clone().unwrap_or_default())
        .unwrap_or_default();

    Ok(message)
}

const CAPACITY: usize = 5;

struct Conversation {
    messages: VecDeque<String>,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting dabot...");

    let bot = Bot::from_env();
    let conversation = Arc::new(Mutex::new(Conversation {
        messages: VecDeque::with_capacity(CAPACITY),
    }));
    let client = Client::new();

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let conversation = conversation.clone();
        let client = client.clone();

        async move {
            let me = bot.get_me().await?;

            let completion = {
                let mut conversation = conversation.lock().await;
                conversation
                    .messages
                    .push_back(msg.text().unwrap_or_default().to_string());

                if conversation.messages.len() > CAPACITY {
                    conversation.messages.pop_front();
                }

                if conversation
                    .messages
                    .back()
                    .map(|msg| msg.contains(&me.mention()))
                    .unwrap_or_default()
                {
                    request_completion(client, &conversation.messages)
                        .await
                        .ok()
                } else {
                    None
                }
            };

            let Some(completion) = completion else {
                return Ok(());
            };

            bot.send_message(msg.chat.id, completion).await?;
            Ok(())
        }
    })
    .await;
}
