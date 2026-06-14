mod command;
mod modal;
mod question_creation_handler;

use std::{str::FromStr, time::Duration};

pub use command::question;

use anyhow::Context as _;
use serenity::{
    all::{
        ButtonStyle, CacheHttp, ComponentInteractionCollector, ComponentInteractionDataKind, Context, CreateActionRow,
        CreateButton, EditInteractionResponse, EditThread, Interaction, UserId,
    },
    builder::CreateComponent,
    model::event::FullEvent,
};
use valine_bot_macros::event_handler;

use crate::{
    app::{AppError, BotDataExt, BotError},
    utils::create_interaction_message,
};

pub static QUESTION_CLOSE_PREFIX: &str = "close_question_forum";

async fn handle_interaction_create(ctx: &Context, interaction: &Interaction) -> Result<(), AppError> {
    let Interaction::Component(interaction) = interaction else {
        return Ok(());
    };
    let ComponentInteractionDataKind::Button = interaction.data.kind else {
        return Ok(());
    };
    let custom_id = &interaction.data.custom_id;
    if !custom_id.starts_with(QUESTION_CLOSE_PREFIX) {
        return Ok(());
    }

    let author_id = custom_id
        .strip_prefix(QUESTION_CLOSE_PREFIX)
        .and_then(|value| value.strip_prefix(':'))
        .ok_or(BotError::InvalidEventData("question close custom ID"))?;
    let author_id = UserId::from_str(author_id)?;
    if author_id != interaction.user.id {
        return Ok(());
    }

    let config = &ctx.app_config().await.question;
    let thread = interaction
        .channel_id
        .expect_thread()
        .to_thread(&ctx, interaction.guild_id)
        .await
        .context("Failed to get question thread")?;
    if thread.applied_tags.contains(&config.solved_tag) {
        interaction
            .create_response(ctx.http(), create_interaction_message("既に解決済みです。", true, None))
            .await
            .context("Failed to send already-solved response")?;
        return Ok(());
    }

    let confirm_custom_id = format!("close_question_confirm:{}", interaction.id);
    let cancel_custom_id = format!("close_question_cancel:{}", interaction.id);

    interaction
        .create_response(
            ctx.http(),
            create_interaction_message(
                "本当に質問を終了しますか？",
                true,
                Some(&[CreateComponent::ActionRow(CreateActionRow::buttons(&[
                    CreateButton::new(&confirm_custom_id)
                        .label("はい")
                        .emoji('✅')
                        .style(ButtonStyle::Danger),
                    CreateButton::new(&cancel_custom_id)
                        .label("いいえ")
                        .emoji('❎')
                        .style(ButtonStyle::Success),
                ]))]),
            ),
        )
        .await
        .context("Failed to send question close confirmation")?;

    let res = ComponentInteractionCollector::new(ctx)
        .custom_ids(
            [confirm_custom_id.clone(), cancel_custom_id]
                .map(|id| id.try_into().unwrap())
                .into(),
        )
        .timeout(Duration::from_secs(60))
        .await;

    let (confirmed, text) = match res {
        Some(i) if i.data.custom_id == confirm_custom_id => (true, "質問を解決済みにしました。"),
        _ => (false, "キャンセルしました。"),
    };

    if confirmed {
        let mut applied_tags = thread.applied_tags.to_vec();
        applied_tags.push(config.solved_tag);

        interaction
            .channel_id
            .expect_thread()
            .edit(
                ctx.http(),
                EditThread::new()
                    .name(format!("{}{}", config.solved_name_prefix, thread.base.name))
                    .applied_tags(applied_tags),
            )
            .await
            .context("Failed to mark question thread as solved")?;
    }

    interaction
        .edit_response(
            ctx.http(),
            EditInteractionResponse::new().content(text).components(vec![]),
        )
        .await
        .context("Failed to update question close response")?;

    Ok(())
}

#[event_handler]
pub async fn handle_question_event(ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
    if let FullEvent::InteractionCreate { interaction, .. } = event {
        handle_interaction_create(ctx, interaction).await?;
    }

    Ok(())
}
