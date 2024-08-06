use std::{collections::VecDeque, sync::Arc};

use async_openai::{
    config::OpenAIConfig,
    error::OpenAIError,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use teloxide::{dispatching::dialogue::GetChatId, prelude::*};
use tokio::sync::Mutex;

const PROMPT: &str = "You are a member of a Telegram group chat between friends. The conversation is casual. \
Everyone speaks Russian and you are expected to speak Russian too. You need to support the conversation by replying with your opinion. Joke sometimes. Use slang. \
Act natural, match the style of the conversation, like punctuation, capitalization, and emojis. Be very short but to the point. Make it no longer than 3 sentences. Appear as if you know \
a lot about the topic. Use Russian swear words like сука or блять where appropriate, when you're disappointed.";

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

struct Conversation {
    messages: VecDeque<String>,
    msgs_since_last_reply: u8,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting dabot...");

    let bot = Bot::from_env();
    let conversation = Arc::new(Mutex::new(Conversation {
        messages: VecDeque::with_capacity(3),
        msgs_since_last_reply: 0,
    }));
    let client = Client::new();

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let conversation = conversation.clone();
        let client = client.clone();

        async move {
            let completion = {
                let mut completion = None;
                let mut conversation = conversation.lock().await;
                conversation
                    .messages
                    .push_back(msg.text().unwrap_or_default().to_string());

                if conversation.messages.len() > 3 {
                    conversation.messages.pop_front();
                }

                let threshold = rand::random::<f64>();
                if threshold >= 0.5
                    || conversation
                        .messages
                        .back()
                        .cloned()
                        .unwrap_or_default()
                        .contains("@rhymedabot")
                {
                    if let Ok(comp) = request_completion(client, &conversation.messages).await {
                        completion = Some(comp);
                    }
                }

                completion
            };

            if let Some(completion) = completion {
                bot.send_message(msg.chat.id, completion).await?;
            }

            Ok(())
        }
    })
    .await;
}
