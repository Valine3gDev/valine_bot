use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("このコマンドの実行に必要なロールがありません。")]
    HasNoRole,
}
