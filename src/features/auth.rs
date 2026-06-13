use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use poise::say_reply;
use rand::seq::IndexedRandom;
use serenity::all::prelude::CacheHttp;
use serenity::all::{
    ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateInputText,
    CreateInteractionResponseFollowup, EmbedMessageBuilding, InputTextStyle, Interaction, Mentionable, MessageBuilder,
    ModalInteractionCollector, UserId,
};
use serenity::async_trait;
use serenity::builder::{CreateComponent, CreateLabel, CreateModalComponent};
use serenity::model::application::{ButtonStyle, LabelComponent, ModalComponent};
use serenity::model::event::FullEvent;
use serenity::small_fixed_array::FixedString;
use tracing::error;

use crate::app::{AppContext, AppError, BotDataGetter};
use crate::core::BotEventHandler;
use crate::utils::{create_ephemeral_message, create_interaction_message, create_message, create_model, send_message};

static KEYWORD_INPUT_BUTTON: &str = "keyword_input:button";
static AUTH_COOLDOWN: Duration = Duration::from_secs(60);

pub struct AuthEventHandler {
    cooldown: Arc<DashMap<UserId, Instant>>,
}

impl AuthEventHandler {
    pub fn new() -> Self {
        Self {
            cooldown: Arc::new(DashMap::new()),
        }
    }

    fn cleanup_cooldown(&self) {
        self.cooldown.retain(|_, instant| instant.elapsed() < AUTH_COOLDOWN);
    }

    fn remaining_cooldown(&self, user_id: UserId) -> Option<u64> {
        self.cleanup_cooldown();

        if let Some(instant) = self.cooldown.get(&user_id) {
            let remaining = AUTH_COOLDOWN.checked_sub(instant.elapsed())?.as_secs();
            if remaining > 0 {
                return Some(remaining);
            }
        }
        None
    }

    fn start_cooldown(&self, user_id: UserId) {
        self.cooldown.insert(user_id, Instant::now());
    }

    async fn handle_interaction_create(&self, ctx: &Context, interaction: &Interaction) {
        let Interaction::Component(interaction) = interaction else {
            return;
        };
        let ComponentInteractionDataKind::Button = interaction.data.kind else {
            return;
        };
        if interaction.data.custom_id != KEYWORD_INPUT_BUTTON {
            return;
        }

        let config = &ctx.read_app_config().await.auth;
        let member = interaction.member.as_ref().unwrap();

        if member.roles.contains(&config.role_id) {
            let _ = interaction
                .create_response(ctx.http(), create_ephemeral_message("既に認証済みです。", None))
                .await;
            return;
        }

        if let Some(remaining) = self.remaining_cooldown(interaction.user.id) {
            let _ = interaction
                .create_response(
                    ctx.http(),
                    create_ephemeral_message(
                        format!("クールダウン中です。\n{remaining}秒後に再度お試しください。"),
                        None,
                    ),
                )
                .await;
            return;
        }

        let mut input_text = CreateInputText::new(InputTextStyle::Short, "keyword")
            .required(true)
            .placeholder("合言葉を入力してください。");

        if let Some(value) = config.dummy_keywords.choose(&mut rand::rng()) {
            input_text = input_text.value(value);
        }

        let custom_id = FixedString::from_str(&interaction.id.to_string()).unwrap();

        let _ = interaction
            .create_response(
                ctx.http(),
                create_model(
                    &custom_id,
                    "合言葉を入力してください。",
                    &[CreateModalComponent::Label(CreateLabel::input_text(
                        "合言葉",
                        input_text,
                    ))],
                ),
            )
            .await;

        let Some(interaction) = ModalInteractionCollector::new(ctx)
            .custom_ids([custom_id].to_vec())
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

        let keyword = if let ModalComponent::Label(label) = interaction.data.components.first().unwrap()
            && let LabelComponent::InputText(text) = label.component.clone()
        {
            text.value
        } else {
            return error!("Invalid modal interaction: {interaction:#?}");
        };
        let keyword = keyword.trim();

        if config.keyword != keyword {
            let _ = interaction
                .create_response(
                    ctx.http(),
                    create_interaction_message("合言葉が間違っています。", true, None),
                )
                .await;
            self.start_cooldown(interaction.user.id);
            return;
        }

        if let Err(why) = member.add_role(ctx.http(), config.role_id, None).await {
            let log = create_message(format!(
                "{} にロールを追加できませんでした。\n```\n{why:#?}```",
                member.mention()
            ));
            let _ = send_message(ctx, &config.log_channel_id, log).await;
            return error!("Failed to add role: {why:#?}");
        }

        let log = create_message(
            MessageBuilder::new()
                .push_named_link_safe(
                    member.display_name(),
                    &*format!("<https://discord.com/users/{}>", member.user.id),
                )
                .push(" (")
                .push_mono(&*member.user.id.to_string())
                .push(") にロールを追加しました。")
                .build(),
        );
        let _ = send_message(ctx, &config.log_channel_id, log).await;

        const AUTH_SUCCESS_MESSAGE: &str = "合言葉を確認しました。\nチャンネルが表示されない場合、アプリの再起動や再読み込み(Ctrl + R)をお試しください。";
        let _ = interaction
            .create_response(ctx.http(), create_interaction_message(AUTH_SUCCESS_MESSAGE, true, None))
            .await;
    }
}

#[async_trait]
impl BotEventHandler for AuthEventHandler {
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
                CreateButton::new(KEYWORD_INPUT_BUTTON)
                    .label(button)
                    .style(ButtonStyle::Primary),
            ]))]),
        )
        .await?;

    Ok(())
}
