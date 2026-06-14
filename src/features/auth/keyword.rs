use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use poise::say_reply;
use rand::seq::IndexedRandom;
use serenity::all::prelude::CacheHttp;
use serenity::all::{
    ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateInputText,
    CreateInteractionResponseFollowup, InputTextStyle, Interaction, Mentionable, ModalInteractionCollector, UserId,
};
use serenity::async_trait;
use serenity::builder::{CreateComponent, CreateLabel, CreateModalComponent};
use serenity::model::application::{ButtonStyle, LabelComponent, ModalComponent};
use serenity::model::colour::colours::branding;
use serenity::model::event::FullEvent;
use serenity::small_fixed_array::FixedString;
use tracing::error;

use crate::app::{AppContext, AppError, BotDataExt};
use crate::core::BotEventHandler;
use crate::features::auth::utils::create_auth_log_message;
use crate::utils::{create_ephemeral_message, create_interaction_message, create_message, create_model, send_message};

const KEYWORD_INPUT_BUTTON_CUSTOM_ID: &str = "keyword_input:button";
const FAILED_ATTEMPT_COOLDOWN: Duration = Duration::from_secs(60);

pub struct KeywordAuthEventHandler {
    cooldown_started_at: Arc<DashMap<UserId, Instant>>,
}

impl KeywordAuthEventHandler {
    pub fn new() -> Self {
        Self {
            cooldown_started_at: Arc::new(DashMap::new()),
        }
    }

    fn remove_expired_cooldowns(&self) {
        self.cooldown_started_at
            .retain(|_, started_at| started_at.elapsed() < FAILED_ATTEMPT_COOLDOWN);
    }

    fn remaining_cooldown_seconds(&self, user_id: UserId) -> Option<u64> {
        self.remove_expired_cooldowns();

        if let Some(started_at) = self.cooldown_started_at.get(&user_id) {
            let remaining_seconds = FAILED_ATTEMPT_COOLDOWN.checked_sub(started_at.elapsed())?.as_secs();
            if remaining_seconds > 0 {
                return Some(remaining_seconds);
            }
        }
        None
    }

    fn start_cooldown_for(&self, user_id: UserId) {
        self.cooldown_started_at.insert(user_id, Instant::now());
    }

    async fn handle_interaction_create(&self, ctx: &Context, interaction: &Interaction) {
        let Interaction::Component(interaction) = interaction else {
            return;
        };
        let ComponentInteractionDataKind::Button = interaction.data.kind else {
            return;
        };
        if interaction.data.custom_id != KEYWORD_INPUT_BUTTON_CUSTOM_ID {
            return;
        }

        let config = &ctx.app_config().await.auth;
        let member = interaction.member.as_ref().unwrap();

        if member.roles.contains(&config.role_id) {
            let _ = interaction
                .create_response(ctx.http(), create_ephemeral_message("既に認証済みです。", None))
                .await;
            return;
        }

        if let Some(remaining_seconds) = self.remaining_cooldown_seconds(interaction.user.id) {
            let _ = interaction
                .create_response(
                    ctx.http(),
                    create_ephemeral_message(
                        format!("クールダウン中です。\n{remaining_seconds}秒後に再度お試しください。"),
                        None,
                    ),
                )
                .await;
            return;
        }

        let mut keyword_input = CreateInputText::new(InputTextStyle::Short, "keyword")
            .required(true)
            .placeholder("合言葉を入力してください。");

        if let Some(value) = config.dummy_keywords.choose(&mut rand::rng()) {
            keyword_input = keyword_input.value(value);
        }

        let modal_custom_id = FixedString::from_str(&interaction.id.to_string()).unwrap();

        let _ = interaction
            .create_response(
                ctx.http(),
                create_model(
                    &modal_custom_id,
                    "合言葉を入力してください。",
                    &[CreateModalComponent::Label(CreateLabel::input_text(
                        "合言葉",
                        keyword_input,
                    ))],
                ),
            )
            .await;

        let Some(interaction) = ModalInteractionCollector::new(ctx)
            .custom_ids([modal_custom_id].to_vec())
            .timeout(Duration::from_secs(60))
            .await
        else {
            let _ = interaction
                .create_followup(
                    ctx.http(),
                    CreateInteractionResponseFollowup::new()
                        .content("時間切れです。もう一度お試しください。")
                        .ephemeral(true),
                )
                .await;
            return;
        };

        let submitted_keyword = if let ModalComponent::Label(label) = interaction.data.components.first().unwrap()
            && let LabelComponent::InputText(text) = label.component.clone()
        {
            text.value
        } else {
            return error!("Invalid modal interaction: {interaction:#?}");
        };
        let submitted_keyword = submitted_keyword.trim();

        if config.keyword != submitted_keyword {
            let _ = interaction
                .create_response(
                    ctx.http(),
                    create_interaction_message("合言葉が間違っています。", true, None),
                )
                .await;
            self.start_cooldown_for(interaction.user.id);
            return;
        }

        if let Err(error) = member.add_role(ctx.http(), config.role_id, Some("認証成功")).await {
            let log = create_message(format!(
                "{} にロールを追加できませんでした。\n```\n{error:#?}```",
                member.mention()
            ));
            let _ = send_message(ctx, &config.log_channel_id, log).await;
            return error!("Failed to add role: {error:#?}");
        }

        let _ = send_message(
            ctx,
            &config.log_channel_id,
            create_auth_log_message("認証成功", branding::GREEN, member, None),
        )
        .await;

        const AUTH_SUCCESS_MESSAGE: &str = "合言葉を確認しました。\nチャンネルが表示されない場合、アプリの再起動や再読み込み(Ctrl + R)をお試しください。";
        let _ = interaction
            .create_response(ctx.http(), create_interaction_message(AUTH_SUCCESS_MESSAGE, true, None))
            .await;
    }
}

#[async_trait]
impl BotEventHandler for KeywordAuthEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        if let FullEvent::InteractionCreate { interaction, .. } = event {
            self.handle_interaction_create(ctx, interaction).await
        }
    }
}

/// 合言葉を入力するボタンを作成します。
#[poise::command(slash_command, ephemeral, guild_only, default_member_permissions = "ADMINISTRATOR")]
pub async fn create_keyword_button(
    ctx: AppContext<'_>,
    #[description = "ボタンの表示名"] button: String,
    #[description = "メッセージ内容"] content: String,
) -> Result<(), AppError> {
    say_reply(ctx, "ボタンを作成しました。").await?;

    let _ = ctx
        .channel_id()
        .send_message(
            ctx.http(),
            create_message(content).components(&[CreateComponent::ActionRow(CreateActionRow::buttons(&[
                CreateButton::new(KEYWORD_INPUT_BUTTON_CUSTOM_ID)
                    .label(button)
                    .style(ButtonStyle::Primary),
            ]))]),
        )
        .await?;

    Ok(())
}
