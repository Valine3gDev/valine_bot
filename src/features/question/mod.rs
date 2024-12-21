mod command;
mod modal;
mod question_creation_handler;

use std::{str::FromStr, time::Duration};

pub use command::question;
use serenity::{
    all::{
        ButtonStyle, CacheHttp, Channel, ComponentInteractionCollector, ComponentInteractionDataKind, Context,
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        EditInteractionResponse, EditThread, EventHandler, Interaction, UserId,
    },
    async_trait,
};
use tracing::error;

use crate::config::get_config;

pub static QUESTION_CLOSE_PREFIX: &str = "close_question_forum";

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Component(interaction) = interaction else {
            return;
        };
        let ComponentInteractionDataKind::Button = interaction.data.kind else {
            return;
        };
        let custom_id = &interaction.data.custom_id;
        if !custom_id.starts_with(QUESTION_CLOSE_PREFIX) {
            return;
        }

        let (_, author_id) = custom_id.split_at(QUESTION_CLOSE_PREFIX.len() + 1);
        let author_id = UserId::from_str(author_id).unwrap();
        if author_id != interaction.user.id {
            return;
        }

        let config = &get_config(&ctx).await.question;
        let Ok(Channel::Guild(channel)) = interaction.channel_id.to_channel(&ctx).await else {
            return error!("Failed to get channel: {:?}", interaction.channel_id);
        };
        if channel.applied_tags.contains(&config.solved_tag) {
            let _ = interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("既に解決済みです。")
                            .ephemeral(true),
                    ),
                )
                .await;
        }

        let confirm_custom_id = format!("close_question_confirm:{}", interaction.id);
        let cancel_custom_id = format!("close_question_cancel:{}", interaction.id);

        let _ = interaction
            .create_response(
                ctx.http(),
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("本当に質問を終了しますか？")
                        .ephemeral(true)
                        .components(
                            [CreateActionRow::Buttons(
                                [
                                    CreateButton::new(&confirm_custom_id)
                                        .label("はい")
                                        .emoji('✅')
                                        .style(ButtonStyle::Danger),
                                    CreateButton::new(&cancel_custom_id)
                                        .label("いいえ")
                                        .emoji('❎')
                                        .style(ButtonStyle::Success),
                                ]
                                .to_vec(),
                            )]
                            .to_vec(),
                        ),
                ),
            )
            .await;

        let res = ComponentInteractionCollector::new(&ctx.shard)
            .custom_ids(vec![confirm_custom_id.clone(), cancel_custom_id.clone()])
            .timeout(Duration::from_secs(60))
            .await;

        let (confirmed, text) = match res {
            Some(i) if i.data.custom_id == confirm_custom_id => (true, "質問を解決済みにしました。"),
            _ => (false, "キャンセルしました。"),
        };

        if confirmed {
            let mut applied_tags = channel.applied_tags.clone();
            applied_tags.push(config.solved_tag);

            interaction
                .channel_id
                .edit_thread(ctx.http(), EditThread::new().applied_tags(applied_tags))
                .await
                .unwrap();
        }

        let _ = interaction
            .edit_response(
                ctx.http(),
                EditInteractionResponse::new().content(text).components(vec![]),
            )
            .await;
    }
}
