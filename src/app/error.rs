use poise::{FrameworkError, say_reply};
use thiserror::Error;
use tracing::error;

use crate::{
    app::{AppError, BotData},
    utils::format_duration,
};

#[derive(Error, Debug)]
pub enum BotError {
    #[error("このコマンドの実行に必要なロールがありません。")]
    HasNoRole,
    #[error("スレッドでのみ実行できるコマンドです。")]
    IsNotInThread,
    #[error("プライベートスレッドでは実行できません。")]
    IsPrivateThread,
}

pub async fn on_error(error: FrameworkError<'_, BotData, AppError>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let _ = say_reply(ctx, "コマンド実行中にエラーが発生しました。").await;
            error!("Command error: Command: {:#?}, Error: {error:#?}", ctx.command());
        }
        FrameworkError::ArgumentParse { ctx, input, error, .. } => {
            let Some(input) = input else {
                return error!("Error parsing input: {error:#?}");
            };

            // let error = match error.downcast_ref::<MessageParseError>() {
            //     Some(MessageParseError::Malformed) => {
            //         "メッセージとして解析できませんでした。\nメッセージID、メッセージURL形式で入力してください。"
            //     }
            //     Some(MessageParseError::Http(_)) => "メッセージを取得できませんでした。",
            //     _ => &error.to_string(),
            // };

            let _ = say_reply(ctx, format!("入力 `{input}` の解析に失敗しました。\n{error:#?}")).await;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let msg = format!("ボットに権限が無いためコマンドを実行できません: {missing_permissions}",);
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
                println!("Error while handling error: {e:#?}")
            }
        }
    }
}
