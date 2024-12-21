use std::{str::FromStr, sync::Arc, time::Duration};

use poise::Modal;
use serenity::{
    all::{
        CacheHttp, CommandInteraction, ComponentInteraction, ComponentInteractionCollector,
        ComponentInteractionDataKind, Context, ForumTagId, ModalInteractionCollector, ModalInteractionData,
    },
    futures::StreamExt,
};
use tokio::sync::{mpsc, RwLock};
use tracing::error;

use super::modal::{BasicQuestionData, DetailedQuestionData};

pub struct CustomIds {
    pub basic: String,
    pub detailed: String,
    pub select_tag: String,
    pub submit: String,
}

impl CustomIds {
    pub fn new(id: u64) -> Self {
        Self {
            basic: format!("open_basic_question_modal:{}", id),
            detailed: format!("open_detailed_question_modal:{}", id),
            select_tag: format!("question_select_tag:{}", id),
            submit: format!("question_submit:{}", id),
        }
    }

    pub fn to_vec(&self) -> Vec<String> {
        vec![
            self.basic.clone(),
            self.detailed.clone(),
            self.select_tag.clone(),
            self.submit.clone(),
        ]
    }
}

static TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct QuestionCreationHandler {
    pub ctx: Context,
    pub interaction: CommandInteraction,
    pub custom_ids: Arc<CustomIds>,
    pub basic_data: Arc<RwLock<Option<BasicQuestionData>>>,
    pub detailed_data: Arc<RwLock<Option<DetailedQuestionData>>>,
    pub forum_tag_ids: Arc<RwLock<Vec<ForumTagId>>>,
    pub submit_tx: mpsc::Sender<()>,
    pub inputted_tx: mpsc::Sender<()>,
}

impl QuestionCreationHandler {
    async fn enable_button(&self) {
        if self.inputted_tx.is_closed() {
            return;
        }

        let has_basic_data = self.basic_data.read().await.is_some();
        let has_detailed_data = self.detailed_data.read().await.is_some();
        let has_forum_tag_ids = !self.forum_tag_ids.try_read().unwrap().is_empty();

        if has_basic_data && has_detailed_data && has_forum_tag_ids {
            self.inputted_tx.send(()).await.unwrap();
        }
    }

    async fn send_modal<M: Modal>(&self, interaction: &ComponentInteraction, default: Option<M>, custom_id: &str) {
        let modal = M::create(default, custom_id.to_owned());
        let Ok(_) = interaction.create_response(self.ctx.http(), modal.clone()).await else {
            error!("Failed to create response: {:?}", modal);
            return;
        };
    }

    async fn parse_response<M: Modal>(&self, data: &ModalInteractionData) -> Option<M> {
        match M::parse(data.clone()) {
            Ok(data) => Some(data),
            Err(e) => {
                error!("Failed to parse modal data: {:?}, {:?}", e, data);
                None
            }
        }
    }

    pub fn handle_component_interaction(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            self_clone._handle_component_interaction().await;
        });
    }

    async fn _handle_component_interaction(self) {
        let mut stream = ComponentInteractionCollector::new(self.ctx.shard.clone())
            .custom_ids(self.custom_ids.to_vec())
            .timeout(TIMEOUT)
            .stream();
        let http = self.ctx.http();
        while let Some(interaction) = stream.next().await {
            match interaction.data.custom_id {
                ref x if x == &self.custom_ids.basic => {
                    self.send_modal::<BasicQuestionData>(
                        &interaction,
                        self.basic_data.read().await.clone(),
                        &self.custom_ids.basic,
                    )
                    .await
                }
                ref x if x == &self.custom_ids.detailed => {
                    self.send_modal::<DetailedQuestionData>(
                        &interaction,
                        Some(self.detailed_data.read().await.clone().unwrap_or_default()),
                        &self.custom_ids.detailed,
                    )
                    .await;
                }
                ref x if x == &self.custom_ids.select_tag => {
                    if let ComponentInteractionDataKind::StringSelect { ref values } = interaction.data.kind {
                        let mut tags = self.forum_tag_ids.write().await;
                        *tags = values.iter().map(|x| ForumTagId::from_str(x).unwrap()).collect();
                        interaction.defer(http).await.unwrap();
                    };
                }
                ref x if x == &self.custom_ids.submit => {
                    self.submit_tx.send(()).await.unwrap();
                    interaction.defer(http).await.unwrap();
                    return;
                }
                _ => {}
            }

            // データが入力されたら送信ボタンを有効化する
            self.enable_button().await;
        }

        let _ = self.interaction.delete_response(http).await;
    }

    pub fn handle_modal_interaction(&self) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            self_clone._handle_modal_interaction().await;
        });
    }

    async fn _handle_modal_interaction(self) {
        let mut stream = ModalInteractionCollector::new(self.ctx.shard.clone())
            .custom_ids(self.custom_ids.to_vec())
            .timeout(TIMEOUT)
            .stream();
        while let Some(res) = tokio::select! {
            res = stream.next() => res,
            _ = self.submit_tx.closed() => None,
        } {
            match res.data.custom_id {
                ref x if x == &self.custom_ids.basic => {
                    let mut data = self.basic_data.write().await;
                    *data = self.parse_response::<BasicQuestionData>(&res.data).await;
                }
                ref x if x == &self.custom_ids.detailed => {
                    let mut data = self.detailed_data.write().await;
                    *data = self.parse_response::<DetailedQuestionData>(&res.data).await;
                }
                _ => {}
            }

            let _ = res.defer(self.ctx.http()).await;

            self.enable_button().await;
        }
    }
}
