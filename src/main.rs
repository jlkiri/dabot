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

const PROMPT: &str = "You are participating in a Telegram group chat conversation. The conversation is casual.\
Everyone speaks Russian and you are expected to speak Russian as well. You need to support the conversation by replying with your opinion or maybe a joke.\
Act natural, match the style of the conversation, like punctuation, capitalization, and emojis. Use emojis a lot. Be very short but to the point. Appear as if you know \
a lot about the topic.";

async fn request_completion(
    cli: Client<OpenAIConfig>,
    conv: Arc<Mutex<Conversation>>,
) -> Result<String, OpenAIError> {
    let mut conversation = conv.lock().await;

    if conversation.msgs_since_last_reply >= 3 {
        conversation.messages.pop_front();
        conversation.msgs_since_last_reply = 0;

        let mut messages = conversation
            .messages
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

        return Ok(message);
    }

    Ok("".to_string())
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
            {
                let mut conversation = conversation.lock().await;
                conversation
                    .messages
                    .push_back(msg.text().unwrap_or_default().to_string());

                conversation.msgs_since_last_reply += 1;
            }

            let completion = request_completion(client, conversation)
                .await
                .expect("Completion went wrong");

            if !completion.is_empty() {
                bot.send_message(msg.chat.id, completion).await?;
            }

            Ok(())
        }
    })
    .await;
}
