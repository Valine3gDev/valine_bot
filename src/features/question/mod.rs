mod command;
mod modal;
mod question_creation_handler;

use std::{str::FromStr, time::Duration};

pub use command::question;

use anyhow::Context as _;
use serenity::{
    all::{
        ButtonStyle, CacheHttp, ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, EditThread,
        Interaction, UserId,
    },
    builder::{CreateComponent, EditInteractionResponse, EditMessage},
    model::{channel::GuildThread, event::FullEvent, id::ForumTagId},
};
use tracing::warn;
use valine_bot_macros::event_handler;

use crate::app::{AppError, BotDataExt};

pub static QUESTION_CLOSE_PREFIX: &str = "close_question_forum";
const QUESTION_THREAD_EDIT_TIMEOUT: Duration = Duration::from_secs(10);

fn parse_question_author_id(custom_id: &str) -> Option<UserId> {
    custom_id
        .strip_prefix(QUESTION_CLOSE_PREFIX)
        .and_then(|value| value.strip_prefix(':'))
        .and_then(|value| UserId::from_str(value).ok())
}

fn toggled_question_name(name: &str, solved_name_prefix: &str, solved: bool) -> String {
    if solved {
        if name.starts_with(solved_name_prefix) {
            name.to_owned()
        } else {
            format!("{solved_name_prefix}{name}")
        }
    } else {
        name.strip_prefix(solved_name_prefix).unwrap_or(name).to_owned()
    }
}

fn toggled_applied_tags(thread: &GuildThread, tag_id: ForumTagId, solved: bool) -> Vec<ForumTagId> {
    let mut applied_tags = thread.applied_tags.to_vec();
    if solved {
        applied_tags.push(tag_id);
    } else {
        applied_tags.retain(|tag| *tag != tag_id);
    }
    applied_tags
}

pub fn create_question_toggle_button(author_id: UserId, solved: bool) -> CreateButton<'static> {
    let button = CreateButton::new(format!("{QUESTION_CLOSE_PREFIX}:{author_id}"));
    if solved {
        button.label("質問を再開する").style(ButtonStyle::Success)
    } else {
        button.label("質問を解決済みにする").style(ButtonStyle::Danger)
    }
}

async fn handle_interaction_create(ctx: &Context, interaction: &Interaction) -> Result<(), AppError> {
    let Interaction::Component(interaction) = interaction else {
        return Ok(());
    };
    let ComponentInteractionDataKind::Button = interaction.data.kind else {
        return Ok(());
    };
    let Some(author_id) = parse_question_author_id(&interaction.data.custom_id) else {
        return Ok(());
    };
    if author_id != interaction.user.id {
        return Ok(());
    }

    interaction
        .defer_ephemeral(ctx.http())
        .await
        .context("Failed to defer question toggle response")?;

    let config = &ctx.app_config().await.question;
    let thread = interaction
        .channel_id
        .expect_thread()
        .to_thread(&ctx, interaction.guild_id)
        .await
        .context("Failed to get question thread")?;

    let next_solved = !thread.applied_tags.contains(&config.solved_tag);

    let edit_thread = EditThread::new()
        .name(toggled_question_name(
            &thread.base.name,
            &config.solved_name_prefix,
            next_solved,
        ))
        .applied_tags(toggled_applied_tags(&thread, config.solved_tag, next_solved));

    let edit_result = tokio::time::timeout(
        QUESTION_THREAD_EDIT_TIMEOUT,
        interaction.channel_id.expect_thread().edit(ctx.http(), edit_thread),
    )
    .await;

    match edit_result {
        Ok(result) => {
            result.with_context(|| format!("Failed to edit thread - solved: {next_solved}"))?;
        }
        Err(_) => {
            warn!(
                "Timed out editing question thread {} after {:?}",
                interaction.channel_id, QUESTION_THREAD_EDIT_TIMEOUT
            );
            interaction
                .edit_response(
                    ctx.http(),
                    EditInteractionResponse::new()
                        .content("チャンネル編集がタイムアウトしました。時間をおいて再度お試しください。"),
                )
                .await
                .context("Failed to send question thread edit timeout response")?;
            return Ok(());
        }
    }

    let mut start_message = (*interaction.message).clone();
    start_message
        .edit(
            &ctx,
            EditMessage::new().components(&[CreateComponent::ActionRow(CreateActionRow::buttons(&[
                create_question_toggle_button(author_id, next_solved),
            ]))]),
        )
        .await
        .context("Failed to update question start message button")?;

    interaction
        .edit_response(
            ctx.http(),
            EditInteractionResponse::new().content(if next_solved {
                "解決済みにしました。"
            } else {
                "再開しました。"
            }),
        )
        .await?;

    Ok(())
}

#[event_handler]
pub async fn handle_question_event(ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
    if let FullEvent::InteractionCreate { interaction, .. } = event {
        handle_interaction_create(ctx, interaction).await?;
    }

    Ok(())
}
