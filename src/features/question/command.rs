use poise::{ApplicationContext, CreateReply};
use serenity::all::{
    ButtonStyle, Channel, CreateActionRow, CreateButton, CreateForumPost, CreateMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, ForumEmoji, ForumTag, ForumTagId, MessageBuilder, ReactionType,
};
use tokio::sync::{RwLock, mpsc};
use tracing::debug;

use std::sync::Arc;
use std::vec;

use crate::config::get_config;
use crate::features::question::QUESTION_CLOSE_PREFIX;
use crate::features::question::modal::{BasicQuestionData, DetailedQuestionData};
use crate::features::question::question_creation_handler::{CustomIds, QuestionCreationHandler};
use crate::utils::has_authed_role;
use crate::{CommandData, PError};

fn reaction_from_forum_emoji(emoji: &ForumEmoji) -> Option<ReactionType> {
    match emoji.clone() {
        ForumEmoji::Id(emoji) => Some(emoji.into()),
        ForumEmoji::Name(emoji) => Some(emoji.try_into().unwrap()),
        _ => None,
    }
}

fn create_select_menu(
    custom_id: impl Into<String>,
    available_tags: &[ForumTag],
    exclude_tags: &[ForumTagId],
    selected_tags: &[ForumTagId],
) -> CreateActionRow {
    let options = available_tags
        .iter()
        .filter(|x| !exclude_tags.contains(&x.id))
        .map(|x| {
            let opt = CreateSelectMenuOption::new(x.name.clone(), x.id.to_string())
                .default_selection(selected_tags.contains(&x.id));
            match &x.emoji {
                Some(emoji) => opt.emoji(reaction_from_forum_emoji(emoji).unwrap()),
                None => opt,
            }
        })
        .collect::<Vec<_>>();

    let select_menu = CreateSelectMenu::new(
        custom_id,
        CreateSelectMenuKind::String {
            options: options.clone(),
        },
    )
    .min_values(1)
    .max_values(options.len().try_into().unwrap())
    .placeholder("タグを選択してください");

    CreateActionRow::SelectMenu(select_menu)
}

/// Modに関する質問を行うためのフォーラムを作成します。
#[poise::command(
    slash_command,
    ephemeral,
    guild_only,
    aliases("質問開始"),
    member_cooldown = 60,
    required_bot_permissions = "CREATE_PUBLIC_THREADS",
    check = "has_authed_role"
)]
pub async fn question(ctx: ApplicationContext<'_, CommandData, PError>) -> Result<(), PError> {
    let custom_ids = Arc::new(CustomIds::new(ctx.id()));

    ctx.defer_ephemeral().await?;

    let submit_button = CreateButton::new(&custom_ids.submit)
        .label("質問を送信")
        .style(ButtonStyle::Success);

    let config = &get_config(ctx.serenity_context()).await.question;
    let Ok(Channel::Guild(channel)) = config.forum_id.to_channel(ctx.serenity_context()).await else {
        return Err("Failed to create forum channel".into());
    };

    let buttons = vec![
        CreateButton::new(&custom_ids.basic)
            .label("質問の基本情報を入力")
            .style(ButtonStyle::Primary),
        CreateButton::new(&custom_ids.detailed)
            .label("質問の詳細情報を入力")
            .style(ButtonStyle::Primary),
        submit_button.clone().disabled(true),
    ];

    const PROMPT: &str = "ボタンをクリックしてすべての情報を入力してください。\nセレクトボックスからタグを設定してください。\nまた、再度ボタンをクリックすると入力内容を編集することができます。";
    let message = ctx
        .send(CreateReply::default().content(PROMPT).components(vec![
            create_select_menu(
                &custom_ids.select_tag,
                &channel.available_tags,
                &config.exclude_tags,
                &[],
            ),
            CreateActionRow::Buttons(buttons.clone()),
        ]))
        .await?;

    let basic_data = Arc::new(RwLock::new(None::<BasicQuestionData>));
    let detailed_data = Arc::new(RwLock::new(None::<DetailedQuestionData>));
    let forum_tag_ids = Arc::new(RwLock::new(Vec::<ForumTagId>::new()));

    let (submit_tx, mut submit_rx) = mpsc::channel::<()>(1);
    let (inputted_tx, mut inputted_rx) = mpsc::channel::<()>(1);

    {
        let handler = QuestionCreationHandler {
            ctx: ctx.serenity_context().clone(),
            interaction: ctx.interaction.clone(),
            custom_ids: custom_ids.clone(),
            basic_data: basic_data.clone(),
            detailed_data: detailed_data.clone(),
            forum_tag_ids: forum_tag_ids.clone(),
            submit_tx,
            inputted_tx,
        };

        handler.handle_component_interaction();
        handler.handle_modal_interaction();
    }

    if inputted_rx.recv().await.is_none() {
        debug!("inputted_rx closed");
        return Ok(());
    }
    inputted_rx.close();

    const CONFIRM: &str = "情報が入力されました、内容を確認し問題なければ「質問を送信」ボタンをクリックしてください。\n### この機能で作成されるフォームは編集出来ません、間違いが無いように気をつけてください。";
    message
        .edit(
            ctx.into(),
            CreateReply::default()
                .content(format!("{}\n{}", PROMPT, CONFIRM))
                .components(vec![
                    create_select_menu(
                        &custom_ids.select_tag,
                        &channel.available_tags,
                        &config.exclude_tags,
                        &forum_tag_ids.read().await,
                    ),
                    CreateActionRow::Buttons({
                        let mut c = buttons.clone();
                        c[2] = submit_button.clone();
                        c
                    }),
                ]),
        )
        .await?;

    if submit_rx.recv().await.is_none() {
        debug!("submit_rx closed");
        return Ok(());
    }
    submit_rx.close();

    let basic_data = basic_data.read().await.clone().unwrap();
    let detailed_data = detailed_data.read().await.clone().unwrap();
    let forum_tag_ids = forum_tag_ids.read().await;

    let msg = MessageBuilder::new()
        .push_line(basic_data.to_string())
        .push_line(detailed_data.to_string())
        .push("\n質問者: ")
        .mention(&ctx.interaction.user)
        .build();

    let forum_channel = channel
        .create_forum_post(
            ctx.http(),
            CreateForumPost::new(
                &basic_data.title,
                CreateMessage::default()
                    .content(msg)
                    .components(vec![CreateActionRow::Buttons(vec![
                        CreateButton::new(format!("{}:{}", QUESTION_CLOSE_PREFIX, ctx.interaction.user.id))
                            .label("質問を解決済みにする")
                            .style(ButtonStyle::Danger),
                    ])]),
            )
            .set_applied_tags(&*forum_tag_ids),
        )
        .await?;

    let msg = MessageBuilder::new()
        .push_line_safe("質問フォーラムを開始しました。")
        .mention(&forum_channel)
        .build();

    message
        .edit(ctx.into(), CreateReply::default().content(msg).components(vec![]))
        .await?;

    Ok(())
}
