use poise::{say_reply, FrameworkError};
use serenity::all::MessageParseError;
use thiserror::Error;
use tracing::error;

use crate::{utils::format_duration, CommandData, PError};

#[derive(Error, Debug)]
pub enum BotError {
    #[error("このコマンドの実行に必要なロールがありません。")]
    HasNoRole,
    #[error("スレッドでのみ実行できるコマンドです。")]
    IsNotInThread,
}

pub async fn on_error(error: FrameworkError<'_, CommandData, PError>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let _ = say_reply(ctx, "コマンド実行中にエラーが発生しました。").await;
            error!("Command error: Command: {:?}, Error: {:?}", ctx.command(), error);
        }
        FrameworkError::ArgumentParse { ctx, input, error, .. } => {
            let Some(input) = input else {
                return error!("Error parsing input: {:?}", error);
            };

            let error = match error.downcast_ref::<MessageParseError>() {
                Some(MessageParseError::Malformed) => {
                    "メッセージとして解析できませんでした。\nメッセージID、メッセージURL形式で入力してください。"
                }
                Some(MessageParseError::Http(_)) => "メッセージを取得できませんでした。",
                _ => &error.to_string(),
            };

            let _ = say_reply(ctx, format!("入力 `{}` の解析に失敗しました。\n{}", input, error)).await;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let msg = format!(
                "ボットに権限が無いためコマンドを実行できません: {}",
                missing_permissions,
            );
            let _ = say_reply(ctx, msg).await;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            let _ = say_reply(ctx, "このコマンドはボットのオーナーのみ実行できます。").await;
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            let _ = say_reply(
                ctx,
                format!(
                    "このコマンドはクールダウン中です。残り時間: {}",
                    format_duration(remaining_cooldown, 2),
                ),
            )
            .await;
        }
        FrameworkError::CommandCheckFailed { ctx, error, .. } => {
            let error = match error {
                Some(error) => error.to_string(),
                None => "コマンドの実行条件を満たしていません。".to_string(),
            };
            let _ = say_reply(ctx, error).await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}
