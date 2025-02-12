use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use poise::say_reply;
use rand::seq::IndexedRandom;
use serenity::all::{
    ActionRowComponent, ButtonStyle, ComponentInteractionDataKind, Context, CreateActionRow, CreateButton,
    CreateInputText, CreateInteractionResponse, CreateInteractionResponseFollowup, CreateModal, EventHandler,
    InputTextStyle, Interaction, Mentionable, ModalInteractionCollector, Ready, UserId,
};
use serenity::async_trait;
use tracing::error;

use crate::config::get_config;
use crate::utils::{create_interaction_message, create_message, send_message};
use crate::{PContext, PError};

static KEYWORD_INPUT_BUTTON: &str = "keyword_input:button";
static AUTH_COOLDOWN: Duration = Duration::from_secs(60);

pub struct Handler {
    cooldown: Arc<DashMap<UserId, Instant>>,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            cooldown: Arc::new(DashMap::new()),
        }
    }

    fn remaining_cooldown(&self, user_id: UserId) -> Option<u64> {
        if let Some(instant) = self.cooldown.get(&user_id) {
            let remaining = (AUTH_COOLDOWN - instant.elapsed()).as_secs();
            if remaining > 0 {
                return Some(remaining);
            }
        }
        None
    }

    fn start_cooldown(&self, user_id: UserId) {
        self.cooldown.insert(user_id, Instant::now());
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Component(interaction) = interaction else {
            return;
        };
        let ComponentInteractionDataKind::Button = interaction.data.kind else {
            return;
        };
        if interaction.data.custom_id != KEYWORD_INPUT_BUTTON {
            return;
        }

        let config = &get_config(&ctx).await.auth;
        let member = interaction.member.as_ref().unwrap();

        if member.roles.contains(&config.role_id) {
            let _ = interaction
                .create_response(&ctx, create_interaction_message("既に認証済みです。", true, None))
                .await;
            return;
        }

        if let Some(remaining) = self.remaining_cooldown(interaction.user.id) {
            let _ = interaction
                .create_response(
                    &ctx,
                    create_interaction_message(
                        format!("クールダウン中です。\n{}秒後に再度お試しください。", remaining),
                        true,
                        None,
                    ),
                )
                .await;
            return;
        }

        let mut input_text = CreateInputText::new(InputTextStyle::Short, "合言葉", "keyword")
            .required(true)
            .placeholder("合言葉を入力してください。");

        if let Some(value) = config.dummy_keywords.choose(&mut rand::rng()) {
            input_text = input_text.value(value);
        }

        let custom_id = interaction.id.to_string();

        let _ = interaction
            .create_response(
                &ctx,
                CreateInteractionResponse::Modal(
                    CreateModal::new(&custom_id, "合言葉を入力してください。")
                        .components([CreateActionRow::InputText(input_text)].to_vec()),
                ),
            )
            .await;

        let Some(interaction) = ModalInteractionCollector::new(&ctx.shard)
            .custom_ids(vec![custom_id])
            .timeout(Duration::from_secs(60))
            .await
        else {
            let _ = interaction
                .create_followup(
                    &ctx,
                    CreateInteractionResponseFollowup::new()
                        .content("時間切れです。もう一度お試しください。")
                        .ephemeral(true),
                )
                .await;
            return;
        };

        let keyword = match interaction.data.components.first().unwrap().components.first() {
            Some(ActionRowComponent::InputText(text)) => text.value.clone().unwrap(),
            _ => return error!("Invalid modal interaction: {:?}", interaction),
        };

        if !config.trigger_regex.is_match(&keyword) {
            let _ = interaction
                .create_response(&ctx, create_interaction_message("合言葉が間違っています。", true, None))
                .await;
            self.start_cooldown(interaction.user.id);
            return;
        }

        if let Err(why) = member.add_role(&ctx, config.role_id).await {
            let log = create_message(format!(
                "{} にロールを追加できませんでした。\n```\n{}```",
                member.mention(),
                why
            ));
            let _ = send_message(&ctx, &config.log_channel_id, log).await;
            return error!("Failed to add role: {:?}", why);
        }

        let log = create_message(format!("{} にロールを追加しました。", member.mention()));
        let _ = send_message(&ctx, &config.log_channel_id, log).await;

        const AUTH_SUCCESS_MESSAGE: &str = "合言葉を確認しました。\nチャンネルが表示されない場合、アプリの再起動や再読み込み(Ctrl + R)をお試しください。";
        let _ = interaction
            .create_response(&ctx, create_interaction_message(AUTH_SUCCESS_MESSAGE, true, None))
            .await;
    }

    async fn ready(&self, _: Context, _: Ready) {
        let cooldown = self.cooldown.clone();
        tokio::spawn(async move {
            loop {
                cooldown.retain(|_, instant| instant.elapsed() < AUTH_COOLDOWN);
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }
}

/// 合言葉を入力するボタンを作成します。
#[poise::command(slash_command, ephemeral, guild_only, default_member_permissions = "ADMINISTRATOR")]
pub async fn create_keyword_button(
    ctx: PContext<'_>,
    #[description = "ボタンの表示名"] button: String,
    #[description = "メッセージ内容"] content: String,
) -> Result<(), PError> {
    say_reply(ctx, "ボタンを作成しました。").await?;

    let _ = ctx
        .channel_id()
        .send_message(
            ctx,
            create_message(content).components(
                [CreateActionRow::Buttons(
                    [CreateButton::new(KEYWORD_INPUT_BUTTON)
                        .label(button)
                        .style(ButtonStyle::Primary)]
                    .to_vec(),
                )]
                .to_vec(),
            ),
        )
        .await?;

    Ok(())
}
